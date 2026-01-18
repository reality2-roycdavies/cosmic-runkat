# Add io.github.reality2_roycdavies.cosmic-runkat

## App Information

**Name:** RunKat
**Summary:** A cute running cat CPU indicator for COSMIC desktop
**License:** MIT
**Homepage:** https://github.com/reality2-roycdavies/cosmic-runkat

## Description

RunKat displays an animated cat in your system tray that runs faster when CPU usage is higher. When CPU usage drops below a configurable threshold, the cat curls up and sleeps.

**Features:**
- Animated running cat that reflects CPU usage
- Optional CPU percentage display beside the cat
- Sleep mode when CPU is idle
- Automatic dark/light theme adaptation
- Panel size awareness (percentage hidden on small panels)
- Configurable sleep threshold

This application was built specifically for the COSMIC desktop environment using libcosmic and Rust.

## Technical Notes

- **Runtime:** org.freedesktop.Platform 24.08
- **Build:** Rust (cargo) with offline build via cargo-sources.json
- **Desktop:** COSMIC only (will not function on other desktop environments)

### Permissions Justification

| Permission | Reason |
|------------|--------|
| `--talk-name=org.kde.StatusNotifierWatcher` | System tray via StatusNotifierItem protocol |
| `--filesystem=host:ro` | Read /proc/stat for CPU monitoring |
| `--filesystem=~/.config/cosmic:ro` | Read COSMIC theme/panel configuration |
| `--filesystem=~/.config/cosmic-runkat:create` | Store app configuration |
| `--filesystem=~/.config/autostart:create` | Create autostart entry for tray |

## Checklist

- [x] App builds successfully with flatpak-builder
- [x] AppStream metainfo.xml is valid
- [x] Desktop file is valid
- [x] Icon is included (SVG)
- [x] Screenshots are included
- [x] OARS content rating is set
- [x] License is open source (MIT)

## Screenshots

![Settings](https://raw.githubusercontent.com/reality2-roycdavies/cosmic-runkat/main/screenshots/settings.png)
