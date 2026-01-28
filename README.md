# Tornade Terminal UI (TUI)

A text-based terminal interface for Tornade music player, built with [ratatui](https://ratatui.rs/).

## Overview

This TUI provides a keyboard-driven interface for browsing and playing music from your library. It uses the same Rust FFI bridge as the SwiftUI interface, demonstrating the multi-UI architecture.

## Features

- 📚 **Library Browser**: Browse all tracks in your library
- 🔍 **Full-Text Search**: FTS5-powered search across tracks
- ▶️ **Playback Control**: Play/pause tracks
- ⌨️ **Keyboard Navigation**: Vim-style keybindings
- 📊 **Library Stats**: View track, album, and artist counts

## Building

```bash
# From project root
cargo build --release -p tornade-tui

# Or from this directory
cargo build --release
```

Binary will be at: `../target/release/tornade-tui`

## Running

```bash
# From project root
cargo run -p tornade-tui

# Or run the binary directly
./target/release/tornade-tui
```

**Note**: Requires a populated music library. If the library is empty, use the Rust API to scan a folder first.

## Keyboard Shortcuts

### Navigation
- `↑` / `k` - Move up
- `↓` / `j` - Move down
- `PageUp` - Move up 10 items
- `PageDown` - Move down 10 items
- `Home` - Jump to first item
- `End` - Jump to last item

### Playback
- `Enter` / `Space` - Play selected track
- `p` - Pause/resume playback

### Search
- `/` - Enter search mode
  - Type to search across tracks
  - `Enter` - Execute search
  - `Esc` - Cancel search

### View
- `1` - Library view (all tracks)
- `2` - Albums view (not yet implemented)

### Other
- `r` - Reload library data
- `?` / `F1` - Show help
- `q` - Quit application
- `Ctrl+C` - Force quit

## Architecture

The TUI directly calls FFI functions from `tornade-core`:

```rust
use tornade_core::ffi;

// Get library statistics
let json = ffi::get_library_stats();

// Load tracks
let json = ffi::get_tracks_page(0, 100);

// Play a track
let json = ffi::play_track(track_id);
```

All data is exchanged as JSON strings, parsed using `serde_json`. This is identical to how the SwiftUI interface communicates with the Rust core.

## Layout

```
┌────────────────────────────────────────────────┐
│  🎵 Tornade TUI - Library                      │
│  42 tracks │ 8 albums │ 5 artists              │
├────────────────────────────────────────────────┤
│                                                │
│  1. Track Name - Artist [3:45] FLAC 16bit... │
│  2. Another Track - Artist [4:20] MP3...      │
│  → 3. Selected Track - Artist [3:30] FLAC...  │ (highlighted)
│  4. More Tracks...                            │
│                                                │
├────────────────────────────────────────────────┤
│  ▶ Playing: Selected Track                    │
│  ↑↓: Navigate │ Enter: Play │ /: Search │ Q: Quit │
└────────────────────────────────────────────────┘
```

## Development

### Dependencies
- **ratatui** (0.26): Terminal UI framework
- **crossterm** (0.27): Cross-platform terminal manipulation
- **tornade-core**: Shared Rust library
- **serde/serde_json**: JSON parsing

### Code Structure
```
src/
├── main.rs    # Terminal setup and event loop
├── app.rs     # Application state and FFI integration
├── ui.rs      # Ratatui UI rendering
└── events.rs  # Keyboard event handling
```

### Adding Features

To add new features:

1. **Add FFI call** in `app.rs`:
   ```rust
   pub fn my_feature(&mut self) {
       let json = ffi::my_function();
       // Parse and handle response
   }
   ```

2. **Add keybinding** in `events.rs`:
   ```rust
   KeyCode::Char('x') => {
       app.my_feature();
   }
   ```

3. **Update UI** in `ui.rs` if needed:
   ```rust
   fn draw_my_widget(f: &mut Frame<B>, app: &App, area: Rect) {
       // Render UI for your feature
   }
   ```

## Limitations

- Player control functions return placeholders (waiting for full FFI implementation)
- Album view not yet implemented
- No queue management UI
- No playlist management

These will be implemented as the FFI bridge is completed for the SwiftUI interface.

## Testing

```bash
# Run with sample library
cargo run -p tornade-tui

# Expected: Should show library stats and track list
# If empty: Use the main Rust API to scan a music folder first
```

## Performance

- Lightweight: ~2MB binary (release mode)
- Low CPU: Only refreshes on input or 250ms timer
- Memory efficient: Loads pages of 100 tracks at a time
- Fast startup: Sub-second initialization

## SSH/Headless Use

Perfect for remote music servers:

```bash
# SSH into server
ssh user@musicserver

# Run TUI
cd /path/to/tornade
./target/release/tornade-tui
```

## Comparison with SwiftUI Interface

| Feature | TUI | SwiftUI |
|---------|-----|---------|
| **Platform** | Any terminal | macOS 13+ |
| **FFI Bridge** | ✅ Same API | ✅ Same API |
| **Library Browser** | ✅ | ✅ |
| **Search** | ✅ | ✅ |
| **Playlists** | ⏳ Planned | ✅ |
| **Album Art** | ❌ N/A | ✅ |
| **Native Dialogs** | ❌ N/A | ✅ |
| **Media Keys** | ❌ | ✅ |

Both interfaces use the **exact same Rust core** - demonstrating the multi-UI architecture.

## Contributing

This TUI serves as a proof-of-concept for the multi-UI architecture. Contributions welcome to:
- Add album/playlist views
- Implement queue management
- Add more keyboard shortcuts
- Improve UI layout and colors
- Add mouse support

## License

Same as main Tornade project.
