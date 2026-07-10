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
    services::{ArtworkService, LibraryService, PlayerService, PlaylistService, SearchService},
    utils::AppPaths,
};

mod app;
mod commands;
mod events;
mod navigation;
mod player;
mod tui_artwork;
mod ui;
mod utils;
mod views;
mod widgets;

use app::AppState;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Initialize application paths and database
    let paths = AppPaths::new()?;
    let pool = db::create_pool(paths.database_path())?;
    db::initialize_database(&pool)?;

    // Initialize services
    let library = LibraryService::new(pool.clone(), paths.clone());
    let player = PlayerService::new(pool.clone())?;
    let playlists = PlaylistService::new(pool.clone());
    let search_svc = SearchService::new(pool.clone());
    let artwork = ArtworkService::new(pool.clone(), paths.clone());

    // Detect terminal image protocol before entering raw mode.
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    // Ghostty: ratatui_image v10 detects Sixel but rendering fails.
    // Kitty uses Unicode placeholders (U=1) which Ghostty doesn't support either.
    // Halfblocks is the only working protocol; font_size from query is preserved.
    if std::env::var("TERM_PROGRAM").is_ok_and(|v| v.eq_ignore_ascii_case("ghostty")) {
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Halfblocks);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Compute target pixel size for grid thumbnails from font metrics
    let tui_target = tui_artwork::tui_target_size(picker.font_size(), 14);

    // Migrate existing artwork to TUI thumbnails in background (non-blocking)
    {
        let paths_clone = paths.clone();
        std::thread::spawn(move || {
            tui_artwork::process_all_pending(&paths_clone, tui_target);
        });
    }

    // Build application state
    let mut app = AppState::new(
        player, library, playlists, search_svc, artwork, paths, picker, tui_target,
    );

    // Run event loop
    let result = run_loop(&mut terminal, &mut app);

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

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
) -> io::Result<()> {
    use std::time::Instant;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();
    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            needs_redraw = false;
        }

        // Poll only for the time remaining until next tick.
        // Use a short timeout while background image loads are in progress so we
        // redraw quickly as each image becomes ready.
        let timeout = if app.has_pending_images {
            Duration::from_millis(30)
        } else {
            tick_rate.saturating_sub(last_tick.elapsed())
        };
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if events::handle_key(app, key) {
                        return Ok(());
                    }
                    needs_redraw = true;
                }
                Event::Mouse(mouse) => {
                    // Only redraw on clicks, not mouse moves (moves fire constantly and are expensive)
                    use ratatui::crossterm::event::MouseEventKind;
                    if matches!(
                        mouse.kind,
                        MouseEventKind::Down(_)
                            | MouseEventKind::Up(_)
                            | MouseEventKind::ScrollDown
                            | MouseEventKind::ScrollUp
                    ) {
                        events::handle_mouse(app, mouse);
                        needs_redraw = true;
                    }
                }
                Event::Resize(_, _) => {
                    needs_redraw = true;
                }
                _ => {}
            }
        } else if app.has_pending_images {
            // Poll timed out while background image loads are in progress: redraw to pick up
            // any newly decoded images that background threads may have pushed to the queue.
            needs_redraw = true;
        }

        if last_tick.elapsed() >= tick_rate {
            if app.tick() {
                needs_redraw = true;
            }
            last_tick = Instant::now();
        }
    }
}
