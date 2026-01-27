# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.4] - 2026-01-27

### Changed

- **Dynamic Theme-Colored Icons**: Icons now use COSMIC theme colors instead of separate light/dark variants
  - Reads foreground color from COSMIC's `background.on` theme value
  - All sprites (cat animation, digits) are dynamically recolored at runtime
  - Provides better integration with custom COSMIC themes

---

## [0.3.3] - 2026-01-27

### Fixed

- **Improved Theme Change Detection**: Added file modification time tracking as backup to inotify
  - Monitors mtime of theme color files (accent, background)
  - More reliable detection of theme changes in all scenarios

---

## [0.3.2] - 2026-01-18

### Changed

- **COSMIC Style Settings**: Redesigned settings window to match COSMIC design language
  - Uses `settings::section()` and `settings::item()` widgets
  - Proper COSMIC-style togglers and sliders with consistent spacing
  - Settings grouped in card-style sections with headers

---

## [0.3.1] - 2026-01-18

### Fixed

- **Flatpak Path Detection**: Fixed COSMIC theme and panel size detection in Flatpak
  - Now correctly reads from host's ~/.config/cosmic instead of sandboxed config
  - Fixes percentage text showing on small panels in Flatpak

- **Theme Detection**: Added periodic polling fallback for theme changes
  - Theme updates now happen within ~1 second instead of relying only on inotify

- **Quit/Restart Reliability**: Improved cleanup when quitting from tray menu
  - Added delay to ensure D-Bus resources are properly released
  - Fixes issue where app wouldn't restart after quitting

### Changed

- Removed "Close" button from settings window (use window controls instead)

---

## [0.3.0] - 2026-01-18

### Changed

- **Flatpak-Ready Architecture**: Refactored for Flatpak sandbox compatibility
  - Removed unused daemon.rs and dbus_client.rs (were never integrated)
  - Updated ksni to 0.3 with `disable_dbus_name()` for PID namespace handling
  - Added XDG autostart entry creation on first tray run

- **Install Script Updates**:
  - Fixed APP_ID to `io.github.reality2_roycdavies.cosmic-runkat`
  - Properly creates autostart entry in `~/.config/autostart/`

### Removed

- Empty `systemd/` directory
- Unused `get_cpu_usage()` function from cpu.rs

---

## [0.2.0] - 2026-01-16

### Added

- Initial Flatpak manifest and packaging files
- COSMIC-only desktop file marker

---

## [0.1.0] - 2026-01-15

### Added

- Initial release
- Animated cat system tray icon
- CPU-based animation speed
- Sleep mode when CPU is idle
- Optional CPU percentage display
- Theme-aware icons (dark/light mode)
- Panel-size aware percentage display
- libcosmic settings application
- Configuration via JSON file
