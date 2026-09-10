use std::io;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui_image::picker::Picker;
use tornade_core::{
    db,
    services::{
        ArtworkService, LibraryService, MetadataEditService, PlayerService, PlaylistService,
        SearchService,
    },
    utils::AppPaths,
};

mod app;
mod async_worker;
mod commands;
mod events;
mod media_keys;
mod navigation;
mod player;
mod tui_artwork;
mod ui;
mod utils;
mod views;
mod wake;
mod widgets;

use app::AppState;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize application paths and database
    let paths = AppPaths::new()?;

    // Route logs to a file instead of stderr: the TUI runs in raw mode /
    // alternate screen, so anything written directly to stderr (e.g. the
    // error!() logs emitted per corrupted file during a scan) visually
    // corrupts the display.
    let log_path = paths.config_dir.join("tornade-tui.log");
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(log_file) => {
            env_logger::Builder::from_default_env()
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .init();
        }
        Err(_) => env_logger::init(),
    }

    let pool = db::create_pool(paths.database_path())?;
    db::initialize_database(&pool)?;

    // Initialize services
    let library = LibraryService::new(pool.clone(), paths.clone());
    let player = PlayerService::new(pool.clone())?;
    let playlists = PlaylistService::new(pool.clone());
    let search_svc = SearchService::new(pool.clone());
    let metadata_edit = MetadataEditService::new(pool.clone());
    let artwork = ArtworkService::new(pool.clone(), paths.clone());

    // Detect terminal image protocol before entering raw mode.
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    // Optional override for crisper artwork on terminals with working pixel
    // graphics: TORNADE_IMG_PROTOCOL=kitty|sixel|iterm2|halfblocks|auto.
    // Halfblocks is blocky (1px per column); Kitty/Sixel/iTerm2 render at full
    // pixel resolution. Ghostty now supports Kitty & Sixel, so users can try
    // `TORNADE_IMG_PROTOCOL=kitty` for sharp images.
    use ratatui_image::picker::ProtocolType;
    match std::env::var("TORNADE_IMG_PROTOCOL")
        .ok()
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("kitty") => picker.set_protocol_type(ProtocolType::Kitty),
        Some("sixel") => picker.set_protocol_type(ProtocolType::Sixel),
        Some("iterm2") => picker.set_protocol_type(ProtocolType::Iterm2),
        Some("halfblocks") => picker.set_protocol_type(ProtocolType::Halfblocks),
        Some("auto") => {} // keep the auto-detected protocol
        _ => {
            // Default: ratatui-image v10's Sixel/Kitty paths are unreliable on
            // Ghostty, so fall back to halfblocks there unless overridden above.
            if std::env::var("TERM_PROGRAM").is_ok_and(|v| v.eq_ignore_ascii_case("ghostty")) {
                picker.set_protocol_type(ProtocolType::Halfblocks);
            }
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Compute target pixel size for grid thumbnails. Generate at ~2x the 9-row
    // display height so the downscale stays sharp (supersampling), which also
    // gives pixel-graphics protocols enough detail to render crisply.
    let tui_target = tui_artwork::tui_target_size(picker.font_size(), 20);

    // Migrate existing artwork to TUI thumbnails in background (non-blocking)
    {
        let paths_clone = paths.clone();
        std::thread::spawn(move || {
            tui_artwork::process_all_pending(&paths_clone, tui_target);
        });
    }

    // Build application state
    let mut app = AppState::new(
        player,
        library,
        playlists,
        search_svc,
        metadata_edit,
        artwork,
        pool.clone(),
        paths,
        picker,
        tui_target,
    );

    // Hardware media keys (macOS/Windows/Linux). Optional: `None` if the OS
    // denies or does not support application media controls (FR-026).
    let (media_tx, media_rx) = std::sync::mpsc::channel();
    let media_handle = media_keys::init(media_tx);

    // Run event loop
    let result = run_loop(&mut terminal, &mut app, media_rx, media_handle);

    // Persist the current track and position so the next launch can resume
    // where the user left off (best-effort, logs and no-ops on failure).
    app.player.save_state();

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }

    Ok(())
}

/// Handle one wake-up. Returns `Ok(true)` when the app should quit.
fn process_wake(app: &mut AppState, w: wake::Wake, needs_redraw: &mut bool) -> io::Result<bool> {
    use ratatui::crossterm::event::MouseEventKind;
    match w {
        wake::Wake::Signal => *needs_redraw = true,
        wake::Wake::Input(Event::Key(key)) => {
            if events::handle_key(app, key) {
                return Ok(true);
            }
            *needs_redraw = true;
        }
        wake::Wake::Input(Event::Mouse(mouse)) => {
            // Only act on clicks/drag/scroll, not moves (moves fire constantly).
            if matches!(
                mouse.kind,
                MouseEventKind::Down(_)
                    | MouseEventKind::Up(_)
                    | MouseEventKind::Drag(_)
                    | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollUp
            ) {
                events::handle_mouse(app, mouse);
                *needs_redraw = true;
            }
        }
        wake::Wake::Input(Event::Resize(_, _)) => *needs_redraw = true,
        wake::Wake::Input(_) => {}
    }
    Ok(false)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
    media_rx: std::sync::mpsc::Receiver<media_keys::MediaKeyEvent>,
    mut media_handle: Option<media_keys::MediaKeyHandle>,
) -> io::Result<()> {
    use std::sync::mpsc::{RecvTimeoutError, channel};
    use std::time::Instant;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();
    let mut needs_redraw = true;
    let mut toggle_debounce = media_keys::Debouncer::new(media_keys::TOGGLE_DEBOUNCE);
    // Last (track_id, is_playing) published to the OS now-playing surface.
    let mut prev_now_playing: Option<(i64, bool)> = None;

    // Event-driven loop: block on a single wake channel instead of polling.
    // A reader thread forwards terminal input; background workers (image decode,
    // async jobs, media keys) call `wake::signal()`. The loop only wakes on a
    // real event or the playback tick, so it never busy-spins and redraws the
    // instant something changes — no image is re-emitted unless it must be.
    let (wake_tx, wake_rx) = channel::<wake::Wake>();
    wake::init(wake_tx.clone());
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if wake_tx.send(wake::Wake::Input(ev)).is_err() {
                break;
            }
        }
    });

    loop {
        if needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            needs_redraw = false;
        }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        match wake_rx.recv_timeout(timeout) {
            Ok(w) => {
                if process_wake(app, w, &mut needs_redraw)? {
                    return Ok(());
                }
                // Coalesce a burst of wakes (e.g. many images finishing at once)
                // into a single redraw.
                while let Ok(w) = wake_rx.try_recv() {
                    if process_wake(app, w, &mut needs_redraw)? {
                        return Ok(());
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }

        // Drain any completed async jobs (online scrape / artwork).
        if app.poll_async() {
            needs_redraw = true;
        }

        // Drain the in-flight background library scan, if any.
        if app.poll_scan() {
            needs_redraw = true;
        }

        // Drain hardware media-key events (FR-024). Toggle is debounced (FR-027).
        while let Ok(ev) = media_rx.try_recv() {
            if matches!(ev, media_keys::MediaKeyEvent::Toggle)
                && !toggle_debounce.accept(Instant::now())
            {
                continue;
            }
            events::handle_media_key(app, ev);
            needs_redraw = true;
        }

        if last_tick.elapsed() >= tick_rate {
            if app.tick() {
                needs_redraw = true;
            }
            last_tick = Instant::now();
        }

        // Publish now-playing state to the OS when the track or play state
        // changes (FR-027a). No-op when media controls are unavailable.
        if let Some(handle) = media_handle.as_mut() {
            let playing = matches!(
                app.player_cache.state,
                tornade_core::services::PlaybackState::Playing
            );
            let current = app
                .player_cache
                .current_track
                .as_ref()
                .map(|t| (t.id, playing));
            if current != prev_now_playing {
                if let Some(track) = app.player_cache.current_track.as_ref() {
                    handle.update_now_playing(
                        &track.title,
                        track.artist_names.first().map(String::as_str),
                        None,
                        Some(track.duration),
                    );
                    handle.set_playback(
                        playing,
                        Duration::from_secs_f64(app.player_cache.position.max(0.0)),
                    );
                } else {
                    handle.set_stopped();
                }
                prev_now_playing = current;
            }
        }
    }
}
