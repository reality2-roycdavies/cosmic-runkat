# Development Notes

This document captures learnings and solutions discovered while developing cosmic-runkat for the COSMIC desktop environment. It serves as both documentation and an educational resource for developers working with similar technologies.

## Table of Contents

1. [System Tray with Animated Icons](#system-tray-with-animated-icons)
2. [Dynamic Icon Composition](#dynamic-icon-composition)
3. [CPU Monitoring and Smoothing](#cpu-monitoring-and-smoothing)
4. [COSMIC Integration](#cosmic-integration)
5. [libcosmic Settings App](#libcosmic-settings-app)
6. [Problem-Solving Journey](#problem-solving-journey)
7. [The v1.0.0 Refactoring Journey](#the-v100-refactoring-journey)
8. [Resources](#resources)

---

## System Tray with Animated Icons

### The Challenge

Creating an animated system tray icon that updates smoothly based on CPU usage, while also displaying a percentage overlay.

### Solution: Embedded Pixmap Icons with Runtime Composition

We use the `ksni` crate's `icon_pixmap()` method to bypass icon theme lookup entirely:

```rust
fn icon_pixmap(&self) -> Vec<ksni::Icon> {
    let img = match self.build_icon() {
        Some(img) => img,
        None => return vec![],
    };

    // Convert RGBA to ARGB (network byte order for D-Bus)
    let mut argb_data = Vec::with_capacity((img.width() * img.height() * 4) as usize);
    for pixel in img.pixels() {
        let [r, g, b, a] = pixel.0;
        argb_data.push(a);
        argb_data.push(r);
        argb_data.push(g);
        argb_data.push(b);
    }

    vec![ksni::Icon {
        width: img.width() as i32,
        height: img.height() as i32,
        data: argb_data,
    }]
}
```

**Key insight:** The StatusNotifierItem protocol expects ARGB byte order, not RGBA. This was a critical discovery from our previous cosmic-bing-wallpaper project.

### Animation Frame Selection

We embed all animation frames at compile time and select based on state. A single set of sprites is used and dynamically recolored to match the COSMIC theme:

```rust
fn get_cat_data(&self) -> &'static [u8] {
    if self.is_sleeping {
        include_bytes!("../resources/cat-sleep.png")
    } else {
        match self.current_frame {
            0 => include_bytes!("../resources/cat-run-0.png"),
            1 => include_bytes!("../resources/cat-run-1.png"),
            // ... 10 frames total
            _ => include_bytes!("../resources/cat-run-0.png"),
        }
    }
}
```

The sprites are then recolored using the theme's foreground color:

```rust
let cat_raw = image::load_from_memory(cat_data).ok()?.to_rgba8();
let cat = recolor_image(&cat_raw, theme_color);
```

---

## Dynamic Icon Composition

### The Challenge

Display the CPU percentage beside the cat, but only when:
- User has enabled it in settings
- Panel size is medium or larger
- Cat is not sleeping

### Solution: Runtime Image Composition

We build the composite icon dynamically:

```rust
fn build_icon(&self) -> Option<RgbaImage> {
    let cat = image::load_from_memory(self.get_cat_data()).ok()?.to_rgba8();

    // Determine if percentage should be shown
    let should_show_pct = self.show_percentage
        && self.panel_medium_or_larger
        && !self.is_sleeping;

    if !should_show_pct {
        // Scale up cat for small panels
        if !self.panel_medium_or_larger {
            return Some(image::imageops::resize(&cat, 48, 48, FilterType::Nearest));
        }
        return Some(cat);
    }

    // Create wider canvas: cat + spacing + percentage
    let pct_width = calculate_percentage_width(self.cpu_percent);
    let total_width = CAT_SIZE + CAT_PCT_SPACING + pct_width;
    let mut icon = RgbaImage::new(total_width, CAT_SIZE);

    // Copy cat to left side
    for (x, y, pixel) in cat.enumerate_pixels() {
        icon.put_pixel(x, y, *pixel);
    }

    // Composite digit sprites for percentage
    let cpu_str = format!("{:.0}", self.cpu_percent);
    let mut x = CAT_SIZE + CAT_PCT_SPACING;
    for ch in cpu_str.chars() {
        if let Some(digit) = load_digit(ch, self.dark_mode) {
            composite_sprite(&mut icon, &digit, x, text_y);
            x += DIGIT_WIDTH + 1;
        }
    }

    Some(icon)
}
```

### Digit Sprites

We created simple 8x12 pixel digit sprites. These are dynamically recolored at runtime using the COSMIC theme's foreground color (read from the `background.on` theme value), ensuring they always match the current theme.

---

## CPU Monitoring and Smoothing

### The Challenge

CPU readings fluctuate rapidly, making the display jittery and hard to read.

### Solution: Moving Average with Smart Sampling

```rust
// CPU smoothing - only add when we get a NEW reading from the monitor
const CPU_SAMPLE_COUNT: usize = 10;
let mut cpu_samples: VecDeque<f32> = VecDeque::with_capacity(CPU_SAMPLE_COUNT);
let mut last_raw_cpu: f32 = -1.0;

// In the main loop:
let new_cpu = cpu_monitor.current();
if (new_cpu - last_raw_cpu).abs() > 0.01 {
    // New reading from monitor (samples every 500ms)
    last_raw_cpu = new_cpu;
    cpu_samples.push_back(new_cpu);
    if cpu_samples.len() > CPU_SAMPLE_COUNT {
        cpu_samples.pop_front();
    }
}

let smoothed_cpu = cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32;
```

**Key insight:** Only add to the sample buffer when the value actually changes (new reading from the monitor), not on every display loop iteration. This gives us a true 5-second moving average (10 samples × 500ms).

### Sleep Threshold with Rounded Comparison

The user pointed out that the sleep/wake transition should use the displayed (rounded) value:

```rust
// Round CPU for display and comparison
let display_cpu = current_cpu.round();

// Sleep check uses rounded value: cat sleeps at 0 to (threshold-1)%
let is_sleeping = display_cpu < config.sleep_threshold;

// Use rounded value for display to match sleep decision
tray.cpu_percent = display_cpu;
```

This ensures consistency: if the display shows 5%, and threshold is 5%, the cat is running.

---

## COSMIC Integration

### Theme Detection

COSMIC stores theme settings in plain text files:

```rust
fn is_dark_mode() -> bool {
    let path = dirs::config_dir()?.join("cosmic/com.system76.CosmicTheme.Mode/v1/is_dark");
    fs::read_to_string(&path).ok()?.trim() == "true"
}
```

### Panel Size Detection

COSMIC stores panel size as a single character (XS, S, M, L, XL):

```rust
fn is_panel_medium_or_larger() -> bool {
    let path = dirs::config_dir()?.join("cosmic/com.system76.CosmicPanel.Panel/v1/size");
    let size = fs::read_to_string(&path).ok()?.trim().to_uppercase();
    matches!(size.as_str(), "M" | "L" | "XL")
}
```

### File Watching for Instant Updates

We use `notify` crate to watch for config changes:

```rust
let mut watcher = RecommendedWatcher::new(
    move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                let _ = tx.send(());
            }
        }
    },
    NotifyConfig::default().with_poll_interval(Duration::from_secs(1)),
)?;

// Watch theme, panel, and app config directories
watcher.watch(theme_dir, RecursiveMode::NonRecursive)?;
watcher.watch(panel_dir, RecursiveMode::NonRecursive)?;
watcher.watch(app_config_dir, RecursiveMode::NonRecursive)?;
```

**Note:** We also poll the app config every 500ms because inotify isn't always reliable for detecting changes to files written atomically (write to temp, rename).

---

## libcosmic Settings App

### The Challenge

Create a native COSMIC settings app that integrates with the desktop's look and feel.

### Solution: libcosmic Application

```rust
impl Application for SettingsApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "io.github.reality2_roycdavies.cosmic-runkat";

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Action<Self::Message>>) {
        (Self { core, config: Config::load() }, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Action<Self::Message>> {
        match message {
            Message::SleepThresholdChanged(value) => {
                self.config.sleep_threshold = value;
                let _ = self.config.save();  // Save immediately for instant tray update
            }
            // ...
        }
        Task::none()
    }

    fn view(&self) -> Element<Self::Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        column()
            .spacing(spacing.space_m)
            .push(/* widgets */)
            .into()
    }
}
```

### Simple Main Structure

Unlike cosmic-bing-wallpaper, cosmic-runkat doesn't need a daemon or complex async runtime. The app has three simple modes:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    match args[1].as_str() {
        "-t" | "--tray" => {
            // Run the system tray with animated cat
            tray::run_tray().map_err(|e| e.into())
        }
        "-s" | "--settings" => {
            // Open settings window (libcosmic has its own runtime)
            settings::run_settings().map_err(|e| e.into())
        }
        // Default (no args): Open settings, start tray first if not running
    }
}
```

The tray reads CPU directly using `systemstat` in a simple synchronous loop - no async runtime needed.

---

## Problem-Solving Journey

### Animation Quality

**Iteration 1:** 5 frames at 48x48 - "looks pixelated and clunky"

**Iteration 2:** 10 frames at 32x32 with simpler design - "better but animation still jerky"

**Iteration 3:** Reduced main loop sleep from 50ms to 16ms - smoother animation

**Iteration 4:** Created bold silhouette design that reads well at small sizes

### CPU Percentage Display

**Iteration 1:** Overlay percentage on cat - "can't read when it overlaps"

**Iteration 2:** Made digits blue - "better but still hard to see"

**Iteration 3:** Moved percentage beside the cat - "much better!"

**Iteration 4:** Hide percentage when cat is sleeping - cleaner look

### Settings Changes Not Reflecting

**Problem:** Changes in settings app weren't showing in tray

**Discovery:** inotify wasn't reliably detecting file changes

**Solution:** Added polling every 500ms as fallback, checking if config values actually changed

### Autostart Race Condition

**Problem:** Tray icon wouldn't appear when autostarting at login, but worked if started manually after login

**Discovery:** StatusNotifierWatcher service wasn't ready when the tray spawned during login. cosmic-bing-wallpaper worked because its D-Bus initialization naturally added enough delay.

**Solution:** Added a 2-second startup delay at the beginning of `run_tray()`:

```rust
pub fn run_tray() -> Result<(), String> {
    // Brief delay on startup to ensure StatusNotifierWatcher is ready
    // This helps when autostarting at login before the panel is fully initialized
    std::thread::sleep(Duration::from_secs(2));

    // ... rest of tray initialization
}
```

---

## Resources

### Crates Used

| Crate | Version | Purpose |
|-------|---------|---------|
| `ksni` | 0.3 | StatusNotifierItem (system tray) |
| `image` | 0.24 | PNG decoding and manipulation |
| `systemstat` | 0.2 | CPU monitoring |
| `libcosmic` | git | COSMIC toolkit for settings app |
| `notify` | 6 | File system watching |
| `dirs` | 6 | XDG directory paths |

### COSMIC Desktop Paths

- Theme config: `~/.config/cosmic/com.system76.CosmicTheme.Mode/v1/is_dark`
- Panel size: `~/.config/cosmic/com.system76.CosmicPanel.Panel/v1/size`

### External Documentation

- [ksni crate docs](https://docs.rs/ksni)
- [image crate docs](https://docs.rs/image)
- [libcosmic source](https://github.com/pop-os/libcosmic)
- [RunCat original](https://github.com/Kyome22/RunCat_for_windows)

---

## Contributing

When adding new features or fixing bugs, please update this document with any learnings that might help future developers.
---

## The v1.0.0 Refactoring Journey

### From Hobbyist to Production Quality

In January 2026, cosmic-runkat underwent a comprehensive refactoring to transform it from functional hobbyist code to production-ready quality. This 10-hour AI-assisted process is extensively documented as a case study in modern software development.

### The Five Phases

**Phase 1: Critical Fixes & Foundations** (1 day)
- Fixed config path inconsistency bug
- Added config validation with auto-fallback
- Created infrastructure modules (paths, constants, error)
- Lockfile race condition fixes
- Tests: 2 → 10

**Phase 2: Performance Optimizations** (1 day)
- Implemented two-level image caching (decode + recolor)
- Eliminated ~240 recolor operations per second
- Optimized hot path operations
- CPU usage: ~0.5-1.0% → ~0.2-0.6%
- Tests: 10 → 16

**Phase 3: Async Architecture** (2-3 days)
- Converted from polling to event-driven with tokio::select!
- Eliminated 16ms busy-wait loop
- Native ksni async API integration
- Structured logging with tracing
- Theme abstraction module
- CPU usage: ~0.2-0.6% → ~0.1-0.2%
- Memory: 298MB → 161MB (46% reduction)
- Tests: 16 → 18

**Phase 4: Error Handling & Robustness** (1 day)
- Fallback icon generation if resources fail
- Process existence check for lockfiles
- User-friendly error messages with troubleshooting tips
- Graceful degradation everywhere
- Tests: 18 → 20

**Phase 5: Testing, Documentation & Educational Materials** (1 day)
- CI/CD pipeline setup
- Comprehensive documentation updates
- AI development case study
- Architecture documentation
- Final polish

### Key Outcomes

**Performance:**
- CPU: 85-95% reduction (1.5-2.0% → 0.1-0.2%)
- Memory: 46% reduction (VSZ)
- Eliminated all polling loops
- Event-driven architecture

**Code Quality:**
- Tests: 2 → 20 (10x increase)
- Modules: 5 → 8 (better organization)
- Error handling: Silent → Comprehensive
- Logging: println! → tracing
- Documentation: Basic → Extensive

**Architecture:**
- Threading: Mixed → Pure async
- Events: Polling → tokio::select!
- Errors: String → Typed (thiserror)
- Coupling: Tight → Abstracted

### Educational Value

This refactoring serves as a comprehensive case study in:
- AI-assisted software transformation
- Rust async/await patterns
- Performance optimization techniques
- Test-driven refactoring
- Progressive enhancement methodology

**Full details:** [AI_DEVELOPMENT_CASE_STUDY.md](AI_DEVELOPMENT_CASE_STUDY.md)
