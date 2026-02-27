# cosmic-runkat v2.0.0 Architecture

## Overview

This document describes the architecture of cosmic-runkat v2.0.0, a native COSMIC panel applet.

**Architecture Type:** Native COSMIC panel applet with iced subscriptions
**Threading Model:** Single-threaded async (libcosmic/iced runtime)
**Communication:** iced subscriptions and messages
**UI Framework:** libcosmic (panel applet + settings window)

---

## System Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    COSMIC Panel Process                       │
│                                                               │
│  ┌──────────────────────────────────────────────────────────┐│
│  │                cosmic-runkat (applet)                     ││
│  │                                                           ││
│  │  ┌─────────────┐    ┌──────────────┐   ┌─────────────┐  ││
│  │  │ Subscriptions│    │  Application │   │  SpriteCache│  ││
│  │  │             │    │    State     │   │             │  ││
│  │  │ • AnimTick  │───▶│ • cpu_%     │◀──│ • 10 frames │  ││
│  │  │   (33ms)    │    │ • frame_idx │   │ • sleep     │  ││
│  │  │ • CpuUpdate │    │ • sleeping  │   │ • themed    │  ││
│  │  │   (500ms)   │    │ • config    │   │             │  ││
│  │  │ • ConfigChk │    │ • popup_id  │   └─────────────┘  ││
│  │  │   (500ms)   │    └──────┬───────┘                    ││
│  │  └─────────────┘           │                             ││
│  │                     ┌──────┴───────┐                     ││
│  │                     │    Views     │                     ││
│  │                     │             │                     ││
│  │                     │ • view()    │ ← panel icon        ││
│  │                     │ • popup()   │ ← dropdown stats    ││
│  │                     └─────────────┘                     ││
│  └──────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────┐
│ cosmic-runkat --settings │  (separate process)
│   libcosmic::Application │
└──────────────────────────┘
```

---

## Module Responsibilities

### applet.rs - Native COSMIC Panel Applet
**Purpose:** Main applet: animation, CPU monitoring, popup display

**Key Components:**
- `RunkatApplet`: cosmic::Application implementation
- `SpriteCache`: Embedded PNG sprites with theme recoloring
- `Message` enum: `AnimationTick`, `CpuUpdate`, `TogglePopup`, `ConfigCheck`, etc.

**Views:**
- `view()`: Cat animation frame in panel via `button_from_element()`
- `popup_content()`: Per-core CPU stats with progress bars, scrollable

**Subscriptions:**
- 33ms animation tick (frame advancement)
- 500ms CPU/freq/temp update
- 500ms config file change check

### main.rs - Entry Point
**Purpose:** CLI parsing, mode selection

**Modes:**
- Default: `cosmic::applet::run::<RunkatApplet>(())`
- `--settings`: Launch settings window
- `--help`, `--version`: Info display

### settings.rs - Settings Application
**Purpose:** libcosmic-based GUI for configuration

**Configuration options:**
- Animation source (CPU usage, frequency, temperature)
- Per-source sleep thresholds
- Show/hide CPU percentage
- FPS range

### config.rs - Configuration Management
**Purpose:** Load, validate, and persist user settings

**Key Features:**
- Per-source sleep thresholds (CPU %, MHz, °C)
- Animation source selection
- Validation with clear error messages

### cpu.rs - CPU Monitoring
**Purpose:** Sample per-core CPU usage from /proc/stat

**Design:** Blocking thread with watch channel, consumed by applet subscription

### sysinfo.rs - CPU Frequency & Temperature
**Purpose:** Read CPU frequency from sysfs and temperature from hwmon

**Sources:**
- `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq`
- `/sys/class/hwmon/hwmon*/temp*_input`

### theme.rs - Theme Detection
**Purpose:** Detect COSMIC theme colors via RON parsing

**Fallback Chain:**
1. Parse COSMIC theme RON files
2. Default gray colors

### constants.rs - Application Constants
**Purpose:** Centralize timing, animation, and validation values

### error.rs - Error Types
**Purpose:** Type-safe error handling with thiserror

---

## Data Flow

### CPU Update Flow

```
/proc/stat → CpuMonitor thread → watch::Sender
                                       ↓
                              iced subscription (500ms)
                                       ↓
                              Message::CpuUpdate(data)
                                       ↓
                              update() → state changes
                                       ↓
                              view() re-renders panel icon
```

### Animation Flow

```
subscription: every 33ms
         ↓
Message::AnimationTick
         ↓
update(): advance frame if enough time elapsed
         (frame rate based on CPU → FPS calculation)
         ↓
view(): render current sprite Handle
         ↓
button_from_element() → panel display
```

### Popup Flow

```
User clicks panel icon
         ↓
Message::TogglePopup
         ↓
get_popup() → COSMIC creates popup window
         ↓
popup_content() renders:
  • Summary bar (overall CPU/freq/temp)
  • Per-core progress bars (scrollable)
  • Settings button
         ↓
User clicks outside → popup dismissed
```

---

## Sprite System

### Loading & Caching

```
Embedded PNGs (include_bytes!)
         ↓
image::load_from_memory() → RgbaImage (10 frames + sleep)
         ↓
recolor_image(theme_color) → themed RgbaImage
         ↓
Handle::from_rgba(w, h, bytes) → iced image Handle
         ↓
Cached in SpriteCache.handles Vec
```

### Theme Recoloring

When the COSMIC theme changes:
1. Detect new accent/text color from theme RON files
2. `SpriteCache::update_colors()` recolors all sprites
3. Rebuild iced `Handle` objects from recolored images
4. Next `view()` call uses updated handles

---

## Architecture Evolution

| Aspect | v1.0.0 (ksni) | v2.0.0 (native applet) |
|--------|---------------|----------------------|
| **Panel Integration** | D-Bus/SNI protocol | Native COSMIC applet API |
| **Popup** | Separate process (layer-shell) | Built-in popup window |
| **Icon Rendering** | ARGB pixel compositing via D-Bus | iced image::Handle in widget tree |
| **CPU % Display** | Digit sprites composited onto icon | Text widget beside cat |
| **Process Model** | Multi-process (tray + popup + settings) | Single process (applet) + settings |
| **Dependencies** | ksni, notify | libcosmic applet feature only |
| **Lines of Code** | ~3000 | ~1600 |

---

## Related Documentation

- **[AI_DEVELOPMENT_CASE_STUDY.md](AI_DEVELOPMENT_CASE_STUDY.md)**: v1.0.0 refactoring case study
- **[DEVELOPMENT.md](DEVELOPMENT.md)**: Implementation techniques and learnings
- **[THEMATIC_ANALYSIS.md](THEMATIC_ANALYSIS.md)**: AI collaboration patterns

---

*Document Version: 2.0*
*Date: 2026-02-06*
*Architecture: cosmic-runkat v2.0.0*
