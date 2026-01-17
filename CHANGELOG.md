# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
