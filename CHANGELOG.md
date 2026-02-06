# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.1.0] - 2026-02-06

### Removed
- Dead code from v1.x: `PopupPosition` enum, `sleep_threshold` legacy field, unused aggregate CPU channel, `animation_metric` field
- Unused `num_cpus` dependency
- `paths.rs` module (Flatpak detection no longer needed)
- Legacy config migration code

### Changed
- All `eprintln!` replaced with structured `tracing` logging
- Refactored to idiomatic Rust: combinator chains, destructuring, `map_err().ok()?` patterns
- Extracted `CpuFrequency::average_mhz()` helper, eliminating 4 inline duplicates
- Extracted `read_khz_as_mhz()` and `read_millidegrees()` helpers in sysinfo
- Consolidated duplicated config dir computation in theme.rs
- Replaced magic numbers with named constants (`POPUP_WIDTH`, `BAR_HEIGHT`, `TEMP_HOT_THRESHOLD`, etc.)
- Simplified README to Flathub-only installation

### Fixed
- Unsafe `unwrap()` on `main_window_id()` replaced with proper guard
- Stale "tray" references updated to "panel" in metainfo and keywords
- Placeholder tests replaced with real assertions

### Documentation
- Comprehensive doc comments added across all modules
- Explains design decisions: blocking I/O thread, watch channels, sprite recoloring, TEA pattern
- Updated screenshots showing all three monitoring modes

---

## [2.0.0] - 2026-02-06

### Added
- Native COSMIC panel applet replacing system tray integration
- Native popup dropdown with per-core CPU stats
- Per-core CPU frequency and temperature display in popup
- Scrollable popup supporting up to 25 rows for many-core CPUs
- Progress bars for frequency and temperature metrics

### Changed
- Complete architecture rewrite from system tray to native COSMIC applet
- Single-process architecture (eliminated D-Bus/SNI dependency)
- Proper panel icon sizing and theme integration
- Simplified codebase: ~1400 fewer lines of code

### Removed
- System tray (ksni) integration
- D-Bus dependency
- Multi-process architecture (separate tray + settings processes)
- Layer-shell popup (replaced by native COSMIC popup)

---

## [1.0.0-alpha.2] - 2026-01-29

### Added
- **Async Event-Driven Architecture**: Complete rewrite of tray loop using tokio::select!
  - Eliminated 16ms polling loop (now purely event-driven)
  - CPU updates trigger via watch::Receiver::changed()
  - Separate async interval timers for animation, config, lockfile
  - 50% CPU reduction vs alpha.1 (now 0.1-0.2%)
  - 46% memory reduction (VSZ: 298MB → 161MB)

- **Structured Logging**: Professional-grade logging with tracing crate
  - Enable debug logs with: `RUST_LOG=cosmic_runkat=debug`
  - Logs startup, CPU updates, config changes, theme changes
  - Helps troubleshoot issues in production

- **Theme Abstraction Module**: Centralized theme detection
  - New theme module for clean separation of concerns
  - Graceful fallback to defaults if theme detection fails
  - Prepared for future libcosmic API integration

### Changed
- Converted from ksni blocking API to native async API
- All sleep() calls converted to tokio::time::sleep()
- Removed file watcher (polling more reliable for theme/config)

### Performance
- **CPU Usage**: 0.2-0.6% → 0.1-0.2% (50% improvement)
- **Memory (VSZ)**: 298MB → 161MB (46% reduction)
- **Update Latency**: 16ms → <1ms (16x faster)

---

## [1.0.0-alpha.1] - 2026-01-29

### Added

- **Config Validation**: Invalid configs auto-fallback to defaults
  - sleep_threshold: 0.0-20.0
  - min_fps/max_fps: 1.0-30.0
  - min_fps must be < max_fps
  - 5 new validation tests

- **Auto-Migration**: Seamless upgrade from 0.3.x
  - Automatically migrates config from legacy location
  - Preserves user settings across major version bump
  - Documented in MIGRATION.md

- **Constants Module**: Centralized all magic numbers
  - All timing values now named constants with rationale
  - Easy to tune and understand
  - Well-documented with explanations

- **Error Infrastructure**: Proper error types with thiserror
  - Type-safe error handling ready for future phases
  - Clear error messages

- **Path Module**: Flatpak-aware path resolution
  - Unified config directory handling
  - Fixes inconsistency between main.rs and config.rs

### Changed

- **Lockfile Timing**: More reliable instance detection
  - Stale threshold: 60s → 45s
  - Refresh interval: 30s → 20s
  - 25-second safety margin prevents race conditions

- **Flatpak Subprocess Spawning**: Fixed tray spawning from settings
  - Uses flatpak-spawn in Flatpak environments
  - Works correctly in both Flatpak and native

### Fixed

- **Critical**: Config path inconsistency in Flatpak (main.rs vs config.rs)
- **Critical**: Tray spawning broken in Flatpak environments
- Lockfile race conditions (60-second window → 25-second margin)

### Performance

- **Image Caching**: Recolored sprites cached separately
  - Eliminates ~240 recolor operations per second
  - Only recolors when theme actually changes
  - 40% CPU reduction
  - 6 new image operation tests

- **Optimized Image Operations**:
  - Direct pixel iteration in recolor_image()
  - Early transparency skip in composite_sprite()
  - Removed global static from hot path

- **CPU Usage**: 0.5-1.0% → 0.2-0.6% (40-60% improvement)

### Documentation

- MIGRATION.md: Comprehensive upgrade guide
- New module documentation with examples
- Detailed commit messages with rationale

### Testing

- Tests increased from 2 → 16 (8x increase)
- Config validation: 5 tests
- Path detection: 3 tests  
- Image operations: 6 tests
- FPS calculations: 2 tests

---

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
