# Session 5: CPU Frequency & Temperature Monitoring (v1.1.0)

**Date:** February 2026
**Duration:** ~4 hours
**Focus:** Adding frequency/temperature monitoring, per-source thresholds, popup improvements

## Summary

This session extended cosmic-runkat from a CPU-only monitor to a multi-metric monitor supporting CPU usage, frequency, and temperature. Key work included:

1. **New sysinfo module** - Reading CPU frequency from `/sys/devices/system/cpu/*/cpufreq/` and temperature from `/sys/class/hwmon/`

2. **Animation source selector** - User can choose what drives the cat animation:
   - CPU Usage (percentage)
   - CPU Frequency (MHz)
   - CPU Temperature (°C)

3. **Per-source thresholds** - Each mode has its own persistent sleep threshold:
   - CPU: 0-30%
   - Frequency: 0 to max MHz
   - Temperature: 20-100°C

4. **Layer-shell popup restoration** - Restored the working layer-shell popup with configurable position (TopLeft, TopRight, BottomLeft, BottomRight)

5. **Bug fixes**:
   - Config validation rejecting valid frequency/temperature thresholds
   - CPU update event overriding animation-source-based sleep state
   - Settings not detecting config changes for new threshold fields
   - Removed non-updating stats from tray menu (ksni caching limitation)

## Key Technical Decisions

### Separate Thresholds Per Mode

**Problem:** Single `sleep_threshold` field didn't make sense across modes with different units.

**Solution:** Added `sleep_threshold_cpu`, `sleep_threshold_freq`, `sleep_threshold_temp` with appropriate defaults and validation ranges.

```rust
pub struct Config {
    pub sleep_threshold_cpu: f32,   // 0-100%
    pub sleep_threshold_freq: f32,  // 0-10000 MHz
    pub sleep_threshold_temp: f32,  // 0-150°C
    // ...
}
```

### Config Validation Fix

**Problem:** Validation only checked legacy `sleep_threshold` against 0-20 range, rejecting valid MHz values.

**Before:**
```rust
if !(0.0..=20.0).contains(&self.sleep_threshold) {
    return Err("invalid");
}
```

**After:**
```rust
if !(0.0..=100.0).contains(&self.sleep_threshold_cpu) { ... }
if !(0.0..=10000.0).contains(&self.sleep_threshold_freq) { ... }
if !(0.0..=150.0).contains(&self.sleep_threshold_temp) { ... }
```

### Sleep State Override Bug

**Problem:** CPU update event was setting `is_sleeping` based on CPU percentage, overriding the animation-source-based setting.

**Fix:** Removed `is_sleeping` update from CPU event; only animation tick sets sleep state based on configured source.

### Frequency Comparison Units

**Problem:** Frequency threshold was being compared as percentage, not MHz.

**Fix:** Compare actual average MHz against threshold in MHz:
```rust
let avg_mhz = freq.per_core.iter().sum::<u32>() as f32 / freq.per_core.len() as f32;
(metric, avg_mhz < config.sleep_threshold_freq)
```

## Debugging Approach

When the cat wasn't sleeping despite frequency being below threshold:

1. Added debug output to see actual comparison values
2. Discovered config validation was rejecting the config and using defaults
3. Fixed validation to allow appropriate ranges
4. Discovered CPU event was overriding sleep state
5. Fixed event handling to only set sleep state in animation tick

## Files Changed

- `src/sysinfo.rs` - New module for frequency/temperature reading
- `src/config.rs` - Per-source thresholds, validation fixes
- `src/settings.rs` - Mode selector, threshold display with units
- `src/tray.rs` - Animation source support, config change detection
- `src/popup.rs` - Layer-shell restoration, per-source stats display
- `Cargo.toml` - Version bump to 1.1.0

## Commits

```
479e218 feat: Add CPU frequency and temperature monitoring options
7e0d934 fix: Remove non-updating stats from tray menu
```

## Lessons Learned

1. **Unit consistency matters** - When comparing values, ensure both sides use the same units (MHz vs %, °C vs %)

2. **Event handlers can conflict** - Multiple handlers updating the same state can cause race conditions; centralize state updates

3. **Config validation must evolve** - When adding new fields with different valid ranges, update validation accordingly

4. **Debug output is invaluable** - When behavior is unexpected, adding temporary debug prints quickly reveals the actual values being compared

5. **ksni menus are cached** - Don't put dynamic data in tray menus; use popup windows for real-time stats instead
