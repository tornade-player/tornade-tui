# tornade-tui

![Tornade Tui Logo](https://github.com/tornade-player/tornade/blob/main/sources/tornade-core-icon.png)

[![CI](https://github.com/tornade-player/tornade-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/tornade-player/tornade-tui/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A terminal UI music player built on [tornade-core](https://github.com/tornade-player/tornade-core). Full feature parity with the macOS GUI: navigate your library, control playback, manage playlists and queue, all from the terminal.

## Features

- **Albums grid** with inline artwork (Kitty, Sixel, iTerm2; halfblocks fallback - see [Image Rendering](#image-rendering))
- **Library views**: Tracks, Albums, Artists, Genres, Playlists, Queue, Search
- **Detail views**: album tracklist, artist albums, genre tracks, playlist editor
- **Player bar**: artwork, transport controls, progress bar, volume, shuffle and repeat indicators
- **Queue management**: add, remove, reorder, clear, play from position
- **Playlist management**: create, rename, delete, add/remove tracks, reorder, import M3U
- **Global search** across tracks, albums, and artists
- **Per-view filter**: press `/` in any list to narrow results in real time
- **Command mode**: press `:` to run commands with autocompletion
- **Track ratings**: press `0`-`5` to set star rating; displayed in all track lists
- **Library scanning** with progress view
- **Mouse support**: sidebar, track list, queue, and player controls
- Built with [ratatui](https://github.com/ratatui-org/ratatui)

## Requirements

- Rust 1.85+ (edition 2024)
- Terminal: any modern terminal; inline artwork requires [Kitty](https://sw.kovidgoyal.net/kitty/), [Ghostty](https://ghostty.org), [iTerm2](https://iterm2.com), or [WezTerm](https://wezfurlong.org/wezterm/)
- Minimum terminal size: 80x24

## Installation

```bash
git clone https://github.com/tornade-player/tornade-tui.git
cd tornade-tui
cargo build --release
./target/release/tornade-tui
```

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `1` | Tracks view |
| `2` | Albums view |
| `3` | Artists view |
| `4` | Genres view |
| `5` | Search view |
| `Tab` | Switch panel focus (cycles search sections in Search view) |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | Open detail / play |
| `q` / `Esc` | Back / quit |
| `?` | Help overlay |

### Playback

| Key | Action |
|-----|--------|
| `Space` | Play / Pause |
| `n` | Next track |
| `N` | Previous track |
| `]` | Seek +10s |
| `[` | Seek -10s |
| `+` | Volume up |
| `-` | Volume down |
| `S` | Toggle shuffle |
| `R` | Cycle repeat (Off / All / One) |

### Library

| Key | Action |
|-----|--------|
| `s` | Scan folder (opens path prompt) |
| `0`-`5` | Rate selected track |
| `/` | Toggle inline filter |
| `a` | Add selected track to queue |
| `A` | Add selected track to a playlist |

### Queue

| Key | Action |
|-----|--------|
| `J` | Move track down |
| `K` | Move track up |
| `x` | Remove track (with confirmation) |
| `X` | Clear queue (with confirmation) |

### Playlists

| Key | Action |
|-----|--------|
| `c` | Create playlist |
| `r` | Rename playlist |
| `d` | Delete playlist (with confirmation) |
| `i` | Import M3U file |

### Command mode

Press `:` to open the command bar. Press `Tab` to autocomplete, `Esc` to cancel.

| Command | Description |
|---------|-------------|
| `:scan <path>` | Scan a folder and add it to the library |
| `:cleanup` | Check for inaccessible sources |
| `:rate <0-5>` | Rate the selected track |
| `:queue add` | Add selected track to queue |
| `:queue clear` | Clear the queue |
| `:playlist create <name>` | Create a new playlist |
| `:playlist delete <name>` | Delete a playlist |
| `:playlist add <name>` | Add selected track to a playlist |
| `:seek <mm:ss>` | Seek to position |
| `:help` | Show help overlay |

## Image Rendering

Album and artist artwork is displayed inline using [ratatui-image](https://github.com/benjajaja/ratatui-image).

**TUI thumbnails**: at startup and after each scan, a background thread generates 128x128 JPEG (quality 45) thumbnails in `~/.config/tornade/assets/tui/{albums,artists}/`. This reduces memory and CPU usage vs loading full-size artwork (~13x smaller files).

**Protocol detection**: `ratatui-image` auto-detects the best image protocol for your terminal (Kitty, Sixel, iTerm2, or halfblocks fallback).

### Known issue: Ghostty + ratatui-image v10

`ratatui-image` v10 detects Sixel on Ghostty, but Sixel rendering fails. Kitty protocol also fails because v10 uses [Unicode placeholders](https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders) (`U=1`), which Ghostty does not support.

**Current workaround**: tornade-tui forces halfblocks on Ghostty (lower resolution, but functional). This is detected via `TERM_PROGRAM=ghostty`.

**Upstream tracking**:
- ratatui-image: needs direct Kitty placement mode (without `U=1`) as alternative
- Ghostty: needs Unicode placeholder support for Kitty graphics protocol

If a future version of ratatui-image or Ghostty resolves this, remove the Ghostty override in `src/main.rs`.

## Related Projects

- [tornade-core](https://github.com/tornade-player/tornade-core) - Rust audio core library (MIT)
- [tornade-gui](https://github.com/tornade-player/tornade-gui) - Native GUI apps for macOS, Windows, Linux (proprietary)

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT - see [LICENSE](LICENSE) for details.
