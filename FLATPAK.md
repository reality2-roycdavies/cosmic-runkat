# Flatpak Packaging for cosmic-runkat

This document describes how to build and publish cosmic-runkat as a Flatpak.

## Prerequisites

1. Install flatpak-builder:
   ```bash
   # Arch/Manjaro
   sudo pacman -S flatpak-builder

   # Fedora
   sudo dnf install flatpak-builder

   # Ubuntu/Debian
   sudo apt install flatpak-builder
   ```

2. Install the Flatpak SDK:
   ```bash
   flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08
   flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
   ```

## Generating cargo-sources.json

The Flatpak build requires a `cargo-sources.json` file that lists all Cargo dependencies for offline building.

1. Clone the flatpak-builder-tools repository:
   ```bash
   git clone https://github.com/flatpak/flatpak-builder-tools.git
   cd flatpak-builder-tools/cargo
   ```

2. Set up Python environment:
   ```bash
   python3 -m venv venv
   source venv/bin/activate
   pip install tomlkit aiohttp PyYAML
   ```

3. Generate the cargo sources:
   ```bash
   python3 flatpak-cargo-generator.py /path/to/cosmic-runkat/Cargo.lock -o /path/to/cosmic-runkat/cargo-sources.json
   ```

## Building the Flatpak

```bash
cd /path/to/cosmic-runkat
flatpak-builder --force-clean build-dir io.github.cosmic-runkat.yml
```

## Testing Locally

```bash
flatpak-builder --user --install --force-clean build-dir io.github.cosmic-runkat.yml
flatpak run io.github.cosmic-runkat
```

## Publishing to Flathub

1. Fork https://github.com/flathub/flathub
2. Create a new branch named `new-pr`
3. Add your manifest as `io.github.cosmic-runkat.yml`
4. Submit a pull request
5. Follow the Flathub review process

See https://github.com/flathub/flathub/wiki/App-Submission for detailed instructions.

## Important Notes

- This application is designed specifically for the COSMIC desktop environment
- It will not function properly on other desktop environments
- The Flatpak has permissions to:
  - Access Wayland/X11 display
  - Access the system tray via StatusNotifierItem
  - Read CPU statistics from `/proc/stat`
  - Read COSMIC configuration from `~/.config/cosmic`
  - Store app configuration in `~/.config/cosmic-runkat`
