# Migration Guide

## cosmic-runkat 1.x → 2.0.0

v2.0.0 is a complete architecture rewrite from a ksni system tray application to a native COSMIC panel applet.

### Breaking Changes

**Removed: System tray mode (`--tray`)**
The app is now a native COSMIC panel applet. There is no `--tray` flag. The applet is launched automatically by the COSMIC panel.

**Removed: Popup process (`--popup`)**
The popup is now built into the applet. No separate popup process is spawned.

**Removed: Autostart**
The applet runs as part of the COSMIC panel — no autostart entry is needed. Remove any old autostart files:
```bash
rm ~/.config/autostart/io.github.reality2_roycdavies.cosmic-runkat.desktop
```

**Removed: D-Bus/SNI dependency**
The `ksni` and `notify` crates have been removed. The applet uses native COSMIC APIs.

**Changed: Installation**
You must now add the applet to your COSMIC panel:
1. Install the binary and desktop entry (see README)
2. Right-click panel → Panel Settings → Add Applet → RunKat

**Unchanged: Config file**
`~/.config/cosmic-runkat/config.json` is fully compatible. No changes needed.

**Unchanged: Settings window**
`cosmic-runkat --settings` still works.

---

## cosmic-runkat 0.3.x → 1.0.0

This section covers the older migration from 0.3.x to 1.0.0.

## Breaking Changes

### 1. Config File Location

**Changed**: Configuration file location now properly respects Flatpak sandbox boundaries.

**0.3.x locations:**
- Native: `~/.config/cosmic-runkat/config.json`
- Flatpak: Varied (bug!)

**1.0.0 locations:**
- Native: `~/.config/cosmic-runkat/config.json` (unchanged)
- Flatpak: `~/.config/cosmic-runkat/config.json` (on host, accessible to sandboxed app)

**Action required:**
- **Native users**: No action needed (automatic migration)
- **Flatpak users**: Config will be auto-migrated on first run if found at old location

### 2. Config Validation

**Changed**: Invalid configuration values are now rejected.

**What was allowed in 0.3.x:**
- Negative sleep thresholds
- `min_fps > max_fps`
- Out-of-range values

**What's enforced in 1.0.0:**
- `sleep_threshold`: 0.0 - 20.0
- `min_fps`: 1.0 - 30.0
- `max_fps`: 1.0 - 30.0
- `min_fps` must be < `max_fps`

**Action required:**
- If you manually edited config.json, ensure values are valid
- Invalid configs will fall back to defaults with a warning

### 3. Lockfile Timing

**Changed**: Lockfiles now refresh every 20 seconds (was 30s) and go stale after 45 seconds (was 60s).

**Impact:**
- More reliable instance detection
- Faster detection of crashed processes

**Action required:** None (internal change)

## New Features in 1.0.0

### Debug Logging

Enable detailed logging for troubleshooting:

```bash
# All debug logs
RUST_LOG=cosmic_runkat=debug cosmic-runkat --tray

# Info level (default)
RUST_LOG=cosmic_runkat=info cosmic-runkat --tray

# Just errors and warnings
RUST_LOG=cosmic_runkat=warn cosmic-runkat --tray
```

### Improved Error Messages

Errors now include helpful troubleshooting tips.

## Troubleshooting

### Tray won't start

**Symptom**: `cosmic-runkat --tray` fails immediately

**Solutions**:
1. Check if COSMIC panel is running: `pgrep cosmic-panel`
2. Restart panel: `systemctl --user restart cosmic-panel`
3. Check logs: `RUST_LOG=cosmic_runkat=debug cosmic-runkat --tray`
4. Check D-Bus: `busctl --user status`

### Config not saving

**Symptom**: Settings changes don't persist

**Solutions**:
1. Check directory permissions: `ls -la ~/.config/cosmic-runkat/`
2. Manually create directory: `mkdir -p ~/.config/cosmic-runkat`
3. Check disk space: `df -h ~`

### Multiple tray icons

**Symptom**: More than one cat icon appears

**Solutions**:
1. Kill all instances: `pkill -f cosmic-runkat`
2. Wait 60 seconds for lockfiles to clear
3. Start tray: `cosmic-runkat --tray`

## Downgrading

If you need to downgrade to 0.3.x:

1. Stop the tray: `pkill -f cosmic-runkat`
2. Backup your config: `cp ~/.config/cosmic-runkat/config.json ~/config-backup.json`
3. Install 0.3.x: `flatpak update io.github.reality2_roycdavies.cosmic-runkat`
4. Restore config if needed: `cp ~/config-backup.json ~/.config/cosmic-runkat/config.json`

## Getting Help

- GitHub Issues: https://github.com/reality2-roycdavies/cosmic-runkat/issues
- With logs: `RUST_LOG=cosmic_runkat=debug cosmic-runkat --tray 2>&1 | tee runkat.log`
