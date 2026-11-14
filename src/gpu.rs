//! GPU load monitoring
//!
//! Reads GPU busy percentage for every GPU the system exposes:
//!
//! - **AMD**: `/sys/class/drm/cardN/device/gpu_busy_percent` plus VRAM from
//!   `mem_info_vram_used` / `mem_info_vram_total`.  Cheap sysfs reads.
//! - **NVIDIA**: `nvidia-smi`, polled every few seconds on a background
//!   thread.  If the card is runtime-suspended (common on hybrid laptops) it
//!   is reported as asleep and *not* queried, because asking nvidia-smi
//!   would wake it up and cost battery.
//! - Other GPUs (e.g. Intel) are listed without a load figure.
//!
//! Polling only happens while [`GpuMonitor::set_active`] is true, which the
//! applet sets when GPU is the selected source.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::constants::NVIDIA_POLL_INTERVAL;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuVendor {
    Amd,
    Nvidia,
    Intel,
    Other,
}

#[derive(Clone, Debug)]
pub struct GpuReading {
    pub name: String,
    /// Busy percentage (0-100), or None when it can't be measured
    pub busy: Option<f32>,
    /// True when the GPU is powered down (runtime-suspended)
    pub asleep: bool,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub gpus: Vec<GpuReading>,
}

impl GpuInfo {
    /// Load of the busiest GPU (0 when none report a load)
    pub fn max_busy(&self) -> f32 {
        self.gpus
            .iter()
            .filter_map(|g| g.busy)
            .fold(0.0, f32::max)
    }
}

/// A DRM card found in sysfs
struct Card {
    device: std::path::PathBuf,
    vendor: GpuVendor,
}

fn find_cards() -> Vec<Card> {
    let mut cards = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return cards;
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        // "card0" but not connectors like "card0-eDP-1"
        .filter(|n| n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit()))
        .collect();
    names.sort_by_key(|n| n[4..].parse::<u32>().unwrap_or(u32::MAX));
    for name in names {
        let device = Path::new("/sys/class/drm").join(&name).join("device");
        let vendor = match read_trimmed(&device.join("vendor")).as_deref() {
            Some("0x1002") => GpuVendor::Amd,
            Some("0x10de") => GpuVendor::Nvidia,
            Some("0x8086") => GpuVendor::Intel,
            _ => GpuVendor::Other,
        };
        cards.push(Card { device, vendor });
    }
    cards
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

/// One row of `nvidia-smi --query-gpu=...` output
#[derive(Clone, Debug, PartialEq)]
struct NvidiaRow {
    name: String,
    busy: f32,
    vram_used_mb: u64,
    vram_total_mb: u64,
}

fn parse_nvidia_smi(output: &str) -> Vec<NvidiaRow> {
    output
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() < 4 {
                return None;
            }
            Some(NvidiaRow {
                busy: f[0].parse().ok()?,
                vram_used_mb: f[1].parse().ok()?,
                vram_total_mb: f[2].parse().ok()?,
                name: f[3].to_string(),
            })
        })
        .collect()
}

fn query_nvidia_smi() -> Vec<NvidiaRow> {
    Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,memory.used,memory.total,name",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_nvidia_smi(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Polls GPUs on a background thread so `nvidia-smi` never blocks the UI.
pub struct GpuMonitor {
    current: Arc<Mutex<GpuInfo>>,
    active: Arc<AtomicBool>,
}

impl GpuMonitor {
    pub fn start() -> Self {
        let current = Arc::new(Mutex::new(GpuInfo::default()));
        let active = Arc::new(AtomicBool::new(false));
        let (out, on) = (current.clone(), active.clone());

        std::thread::Builder::new()
            .name("runkat-gpu".into())
            .spawn(move || {
                let mut nvidia_rows: Vec<NvidiaRow> = Vec::new();
                let mut last_nvidia: Option<Instant> = None;
                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    if !on.load(Ordering::Relaxed) {
                        last_nvidia = None;
                        continue;
                    }

                    let cards = find_cards();
                    let nvidia_awake = cards.iter().any(|c| {
                        c.vendor == GpuVendor::Nvidia
                            && read_trimmed(&c.device.join("power/runtime_status")).as_deref()
                                != Some("suspended")
                    });
                    if nvidia_awake
                        && last_nvidia.is_none_or(|t| t.elapsed() >= NVIDIA_POLL_INTERVAL)
                    {
                        nvidia_rows = query_nvidia_smi();
                        last_nvidia = Some(Instant::now());
                    }

                    let mut gpus = Vec::new();
                    let mut nvidia_index = 0;
                    for card in &cards {
                        gpus.push(read_card(card, &nvidia_rows, &mut nvidia_index));
                    }
                    if let Ok(mut guard) = out.lock() {
                        *guard = GpuInfo { gpus };
                    }
                }
            })
            .expect("failed to spawn GPU monitor thread");

        Self { current, active }
    }

    /// Turn polling on or off
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    pub fn current(&self) -> GpuInfo {
        self.current.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

fn read_card(card: &Card, nvidia_rows: &[NvidiaRow], nvidia_index: &mut usize) -> GpuReading {
    let mb = |p: &str| read_u64(&card.device.join(p)).map(|b| b / 1024 / 1024);
    match card.vendor {
        GpuVendor::Amd => GpuReading {
            name: "AMD GPU".into(),
            busy: read_u64(&card.device.join("gpu_busy_percent")).map(|v| v as f32),
            asleep: false,
            vram_used_mb: mb("mem_info_vram_used"),
            vram_total_mb: mb("mem_info_vram_total"),
        },
        GpuVendor::Nvidia => {
            let asleep = read_trimmed(&card.device.join("power/runtime_status")).as_deref()
                == Some("suspended");
            let row = nvidia_rows.get(*nvidia_index);
            *nvidia_index += 1;
            GpuReading {
                name: row.map_or("NVIDIA GPU".into(), |r| r.name.clone()),
                busy: if asleep { Some(0.0) } else { row.map(|r| r.busy) },
                asleep,
                vram_used_mb: row.filter(|_| !asleep).map(|r| r.vram_used_mb),
                vram_total_mb: row.map(|r| r.vram_total_mb),
            }
        }
        GpuVendor::Intel | GpuVendor::Other => GpuReading {
            name: if card.vendor == GpuVendor::Intel { "Intel GPU" } else { "GPU" }.into(),
            busy: None,
            asleep: false,
            vram_used_mb: None,
            vram_total_mb: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_rows() {
        let rows = parse_nvidia_smi("7, 512, 4096, NVIDIA GeForce GTX 1650\n");
        assert_eq!(
            rows,
            vec![NvidiaRow {
                name: "NVIDIA GeForce GTX 1650".into(),
                busy: 7.0,
                vram_used_mb: 512,
                vram_total_mb: 4096,
            }]
        );
    }

    #[test]
    fn skips_unparseable_nvidia_rows() {
        assert!(parse_nvidia_smi("[N/A], 1, 2, X\ngarbage\n").is_empty());
    }

    #[test]
    fn max_busy_ignores_unknown() {
        let g = |busy| GpuReading {
            name: String::new(),
            busy,
            asleep: false,
            vram_used_mb: None,
            vram_total_mb: None,
        };
        let info = GpuInfo { gpus: vec![g(None), g(Some(12.0)), g(Some(40.0))] };
        assert_eq!(info.max_busy(), 40.0);
    }
}
