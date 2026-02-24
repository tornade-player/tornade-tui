# tornade-tui

[![CI](https://github.com/tornade-player/tornade-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/tornade-player/tornade-tui/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A terminal UI music player built on [tornade-core](https://github.com/tornade-player/tornade-core). Navigate your music library, control playback, and manage playlists — all from the terminal.

## Features

- Browse music library by album, artist, and playlist
- Full playback control (play, pause, next, previous, seek)
- Shuffle and repeat modes
- Queue management
- Keyboard-driven interface built with [ratatui](https://github.com/ratatui-org/ratatui)

## Installation

### From source

```bash
git clone https://github.com/tornade-player/tornade-tui.git
cd tornade-tui
cargo build --release
./target/release/tornade-tui
```

### Via cargo install

```bash
cargo install --git https://github.com/tornade-player/tornade-tui
```

## Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Space` | Play / Pause |
| `n` | Next track |
| `p` | Previous track |
| `s` | Toggle shuffle |
| `r` | Toggle repeat |
| `↑` / `↓` | Navigate list |
| `Enter` | Select / Play |
| `Tab` | Switch panel |
| `/` | Search |

## Related Projects

- [tornade-core](https://github.com/tornade-player/tornade-core) — Rust audio core library (MIT)
- [tornade-gui](https://github.com/tornade-player/tornade-gui) — Native GUI apps for macOS, Windows, Linux (proprietary)

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT — see [LICENSE](LICENSE) for details.
