use std::io;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use tornade_core::{
    db,
    services::{ArtworkService, LibraryService, PlaylistService, PlayerService, SearchService},
    utils::AppPaths,
};

mod app;
mod commands;
mod events;
mod navigation;
mod player;
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
    let artwork = ArtworkService::new(pool.clone(), paths);

    // Build application state
    let mut app = AppState::new(player, library, playlists, search_svc, artwork);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if events::handle_key(app, key) {
                    return Ok(());
                }
            }
        }

        app.tick();
    }
}
