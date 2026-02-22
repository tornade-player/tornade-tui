// Event handling for keyboard input
// Arrow keys, space, /, q, etc.

use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Search mode handling
    if app.search_mode {
        match key.code {
            KeyCode::Esc => {
                app.search_mode = false;
                app.search_query.clear();
                app.status_message = String::from("Search cancelled");
            }
            KeyCode::Enter => {
                app.search_mode = false;
                app.search();
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
            }
            _ => {}
        }
        return;
    }

    // Normal mode handling
    match key.code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.previous_item();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_item();
        }
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.previous_item();
            }
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.next_item();
            }
        }
        KeyCode::Home => {
            app.selected_index = 0;
        }
        KeyCode::End => {
            if !app.tracks.is_empty() {
                app.selected_index = app.tracks.len() - 1;
            }
        }

        // Playback controls
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.play_selected();
        }
        KeyCode::Char('p') => {
            app.toggle_playback();
        }

        // Search
        KeyCode::Char('/') => {
            app.search_mode = true;
            app.search_query.clear();
            app.status_message = String::from("Enter search query...");
        }

        // Reload
        KeyCode::Char('r') => {
            app.load_tracks();
            app.load_stats();
        }

        // View switching
        KeyCode::Char('1') => {
            app.view = crate::app::View::Library;
            app.load_tracks();
        }
        KeyCode::Char('2') => {
            app.view = crate::app::View::Albums;
            app.status_message = String::from("Album view not yet implemented");
        }

        // Help
        KeyCode::Char('?') | KeyCode::F(1) => {
            app.status_message = String::from(
                "Controls: ↑↓/jk=Navigate │ Enter/Space=Play │ p=Pause │ /=Search │ r=Reload │ q=Quit",
            );
        }

        // Ctrl+C for force quit
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // This will be handled by main loop
        }

        _ => {}
    }
}
