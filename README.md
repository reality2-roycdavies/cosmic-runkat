# cosmic-runkat

A cute running cat CPU indicator for the [COSMIC desktop environment](https://system76.com/cosmic) on Linux. Inspired by [RunCat](https://github.com/Kyome22/RunCat365).

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-2021-orange.svg)
![COSMIC](https://img.shields.io/badge/desktop-COSMIC-purple.svg)

## Features

- **Animated Cat**: A pixel art cat runs in your system tray
- **CPU-Based Speed**: The cat runs faster when CPU usage is higher
- **Sleep Mode**: Cat curls up and sleeps when CPU drops below a configurable threshold
- **CPU Percentage Display**: Optional percentage overlay on the tray icon
- **Theme-Aware**: Automatically adapts icons to dark/light mode
- **Instant Theme Detection**: Uses inotify file watching for immediate theme changes

## Architecture

The application uses a daemon+clients architecture:

```
                    D-Bus (org.cosmicrunkat.Service1)
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   Settings App          Daemon              Tray Client
   (settings)          (daemon.rs)           (tray.rs)
```

- **Daemon**: Background service monitoring CPU usage and managing configuration
- **Tray Client**: Animated system tray icon with dynamic icon composition
- **Settings App**: Configuration UI (coming soon)

## Installation

### Build from Source

```bash
# Clone the repository
git clone https://github.com/reality2-roycdavies/cosmic-runkat.git
cd cosmic-runkat

# Build
cargo build --release

# Run the tray
./target/release/cosmic-runkat --tray
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

## Usage

```bash
# Run the system tray (default)
cosmic-runkat

# Or explicitly
cosmic-runkat --tray

# Run the D-Bus daemon
cosmic-runkat --daemon

# Open settings
cosmic-runkat --settings

# Show help
cosmic-runkat --help
```

## Configuration

Configuration is stored at `~/.config/cosmic-runkat/config.json`:

```json
{
  "sleep_threshold": 5.0,
  "max_fps": 10.0,
  "min_fps": 1.0,
  "show_percentage": true
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `sleep_threshold` | CPU % below which the cat sleeps | 5.0 |
| `max_fps` | Maximum animation speed (frames/sec) | 10.0 |
| `min_fps` | Minimum animation speed when running | 1.0 |
| `show_percentage` | Show CPU % on the tray icon | true |

## How It Works

1. **CPU Monitoring**: Uses `systemstat` crate to sample CPU usage every 500ms
2. **Animation Speed**: Linear interpolation between min/max FPS based on CPU %
3. **Icon Composition**: Cat animation frames + digit sprites are composited at runtime
4. **Theme Detection**: Watches COSMIC's theme config file for instant dark/light switching

## Credits

- Inspired by [RunCat](https://github.com/Kyome22/RunCat365) by Kyome22
- Built for [COSMIC Desktop](https://github.com/pop-os/cosmic-epoch) by System76

## License

MIT License - See [LICENSE](LICENSE) for details.

---

*This project was developed collaboratively by Dr. Roy C. Davies and Claude (Anthropic's AI) using Claude Code.*
