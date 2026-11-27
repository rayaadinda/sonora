# SONORA

```text
███████╗ ██████╗ ███╗   ██╗ ██████╗ ██████╗  █████╗
██╔════╝██╔═══██╗████╗  ██║██╔═══██╗██╔══██╗██╔══██╗
███████╗██║   ██║██╔██╗ ██║██║   ██║██████╔╝███████║
╚════██║██║   ██║██║╚██╗██║██║   ██║██╔══██╗██╔══██║
███████║╚██████╔╝██║ ╚████║╚██████╔╝██║  ██║██║  ██║
╚══════╝ ╚═════╝ ╚═╝  ╚═══╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝
```

Audiophile lossless **FLAC** terminal music player built with `ratatui`, `crossterm`, `rodio`, `lofty`, and `rustfft`.

## Features

- **Lossless FLAC Playback & Seeking**: Symphonia FLAC decoding with smooth forward/backward seeking and live audio quality badges (bit depth, sample rate, bitrate, channels).
- **Real-Time FFT Visualizer**: Three live modes (`Spectrum` with peak-hold caps, `Mirror`, and `Wave`) driven directly by tapped PCM samples.
- **Embedded Album Cover Art**: True-color Unicode half-block (`▀`) rendering for embedded FLAC cover images, with an animated vinyl fallback.
- **Monochrome Themes**: `Obsidian` (default), `Graphite`, `Noir`, and `Paper` (light monochrome) with subtle grayscale gradients.
- **Non-Blocking Library Scanner & Live Search**: Background recursive `.flac` scanning with instant `/` filtering by title, artist, or album.
- **Discord Rich Presence**: Displays **`Listening to Sonora`** with live song progress bar, artist/title, lossless quality tooltip, and automatic album cover art lookup (toggle anytime with `D`).
- **Persistent Settings**: Saves theme, volume, shuffle, repeat, visualizer mode, Discord RPC preference, and folders to `sonora/config.toml`.

## Installation (Windows)

Download the latest release from **GitHub Releases**:

- **Windows Installer (`sonora-v1.0.0-windows-x86_64-setup.exe`)**: Installs Sonora, creates a Start Menu shortcut, and optionally adds `sonora` to your `PATH`.
- **Portable Archive (`sonora-v1.0.0-windows-x86_64-portable.zip`)**: Extract and run `sonora.exe` directly.

## Build & Run from Source

```powershell
cargo check
cargo clippy --all-targets -- -D warnings
cargo run --release -- "C:\Path\To\Your\FLAC\Folder"
```

Run without arguments to scan your default OS Music directory (or saved folders):

```powershell
cargo run --release
```

### Build the Windows Installer Locally

Requires [Inno Setup 6](https://jrsoftware.org/isinfo.php):

```powershell
cargo build --release
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /DAppVersion=1.0.0 installer\sonora.iss
```

## Keybindings

| Key | Action |
| :--- | :--- |
| `↑` / `k`, `↓` / `j` | Move track selection |
| `g` / `G` | Jump to top / bottom |
| `Enter` | Play selected track |
| `Space` | Play / Pause |
| `←` / `h`, `→` / `l` | Seek backward / forward 5s |
| `n` / `p` | Next / Previous track |
| `+` / `=` , `-` / `_` | Increase / Decrease volume |
| `m` | Toggle mute |
| `s` | Toggle shuffle |
| `r` | Cycle repeat (`Off` / `All` / `One`) |
| `o` | Open / Add / Remove music folders |
| `R` | Rescan all music folders |
| `/` | Filter library live |
| `t` | Cycle color theme |
| `v` | Cycle visualizer (`Spectrum` / `Mirror` / `Wave`) |
| `D` | Toggle Discord Rich Presence |
| `?` | Toggle help overlay |
| `q` / `Ctrl+C` | Quit |
