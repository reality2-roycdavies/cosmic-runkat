# cosmic-runkat

A cute running cat CPU indicator for the [COSMIC desktop environment](https://system76.com/cosmic) on Linux. Inspired by [RunCat](https://github.com/Kyome22/RunCat_for_windows) by Kyome22.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-2021-orange.svg)
![COSMIC](https://img.shields.io/badge/desktop-COSMIC-purple.svg)

## About This Project

This project was developed as an **educational exercise** in collaborative AI-assisted software development, following the same approach used for [cosmic-bing-wallpaper](https://github.com/reality2-roycdavies/cosmic-bing-wallpaper). The entire application was built through iterative conversation between a human developer and Claude (Anthropic's AI assistant) using Claude Code.

**Development time:** Approximately 3 hours from initial concept to working release, plus ~2 hours for Flatpak refactoring.

The development process, including all conversations and design decisions, has been documented in the [docs/](docs/) directory for anyone interested in understanding how AI-assisted development works in practice.

### Credits

- **Original Inspiration**: [RunCat](https://github.com/Kyome22/RunCat_for_windows) by Kyome22 - A delightful macOS/Windows app that shows a running cat in the menu bar whose speed reflects CPU usage
- **COSMIC Desktop**: [System76's COSMIC](https://github.com/pop-os/cosmic-epoch) - The next-generation Linux desktop environment
- **Development**: Dr. Roy C. Davies and Claude (Anthropic's AI) using Claude Code

## Features

- **Animated Cat**: A pixel art cat runs in your system tray
- **CPU-Based Speed**: The cat runs faster when CPU usage is higher
- **Sleep Mode**: Cat curls up and sleeps when CPU drops below a configurable threshold
- **CPU Percentage Display**: Optional percentage shown beside the cat (hidden when sleeping)
- **Theme-Aware**: Automatically adapts icons to dark/light mode
- **Panel-Size Aware**: Percentage only shown on medium or larger panels
- **Instant Updates**: Uses inotify file watching for immediate theme/config changes
- **Settings App**: libcosmic-based settings window for configuration

## Installation

### Option 1: Flatpak (Recommended)

```bash
# Install from Flathub (when available)
flatpak install flathub io.github.reality2_roycdavies.cosmic-runkat

# Run
flatpak run io.github.reality2_roycdavies.cosmic-runkat --tray
```

### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/reality2-roycdavies/cosmic-runkat.git
cd cosmic-runkat

# Install (builds and installs to ~/.local)
./install.sh

# Or install system-wide
sudo ./install.sh
```

### Dependencies

**Build Dependencies** (Pop!_OS/Ubuntu/Debian):
```bash
sudo apt install -y \
    build-essential \
    cargo \
    cmake \
    libdbus-1-dev \
    libexpat1-dev \
    libfontconfig-dev \
    libfreetype-dev \
    libxkbcommon-dev \
    pkg-config
```

**Arch Linux/Manjaro**:
```bash
sudo pacman -S base-devel cargo cmake dbus expat fontconfig freetype2 libxkbcommon
```

## Usage

```bash
# Run the system tray (default)
cosmic-runkat

# Or explicitly
cosmic-runkat --tray

# Open settings
cosmic-runkat --settings

# Show help
cosmic-runkat --help
```

The tray app automatically creates an XDG autostart entry on first run, so it starts on login.

## Configuration

Configuration is stored at `~/.config/cosmic-runkat/config.json`:

```json
{
  "sleep_threshold": 5.0,
  "max_fps": 15.0,
  "min_fps": 2.0,
  "show_percentage": true
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `sleep_threshold` | CPU % below which the cat sleeps | 5.0 |
| `max_fps` | Maximum animation speed (frames/sec) | 15.0 |
| `min_fps` | Minimum animation speed when running | 2.0 |
| `show_percentage` | Show CPU % beside the cat | true |

Settings can also be changed via the Settings app (right-click tray icon > Settings).

## How It Works

1. **CPU Monitoring**: Uses `systemstat` crate to sample CPU usage every 500ms
2. **Smoothing**: 10-sample moving average (5 seconds) for stable readings
3. **Animation Speed**: Linear interpolation between min/max FPS based on CPU %
4. **Sleep Logic**: Cat sleeps when rounded CPU % is below threshold
5. **Icon Composition**: Cat animation frames + digit sprites composited at runtime
6. **Theme Detection**: Watches COSMIC's theme config file for instant dark/light switching
7. **Panel Detection**: Reads COSMIC panel size config to show/hide percentage

## Architecture

```
cosmic-runkat
├── src/
│   ├── main.rs        # Entry point, CLI parsing, autostart setup
│   ├── tray.rs        # System tray with animated icon
│   ├── settings.rs    # libcosmic settings app
│   ├── config.rs      # Configuration management
│   └── cpu.rs         # CPU monitoring
├── resources/         # PNG sprites (cat frames, digits)
├── docs/              # Development documentation
└── install.sh         # Installation script
```

## Development Documentation

The [docs/](docs/) directory contains:

- **DEVELOPMENT.md**: Technical notes and learnings from the development process
- **THEMATIC_ANALYSIS.md**: Analysis of themes and patterns from the AI-assisted development
- **transcripts/**: Complete conversation transcripts for educational purposes

## Uninstalling

```bash
./install.sh --uninstall
# Or system-wide
sudo ./install.sh --uninstall
```

## License

MIT License - See [LICENSE](LICENSE) for details.

## Related Projects

- [cosmic-bing-wallpaper](https://github.com/reality2-roycdavies/cosmic-bing-wallpaper) - Another COSMIC app developed with the same AI-assisted approach
- [RunCat](https://github.com/Kyome22/RunCat_for_windows) - The original inspiration for this project

---

*This project was developed collaboratively by Dr. Roy C. Davies and Claude (Anthropic's AI) using Claude Code. See the [docs/](docs/) directory for the full development story.*
