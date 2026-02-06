//! System information module for CPU frequency and temperature
//!
//! Reads per-core CPU frequency from Linux sysfs and CPU temperature from
//! hwmon.  These paths are Linux-specific; on other platforms the functions
//! return empty/default data (the applet will still work, just without
//! frequency or temperature information).
//!
//! ## sysfs paths used
//!
//! - `/sys/devices/system/cpu/cpu{N}/cpufreq/scaling_cur_freq` — current
//!   frequency in kHz
//! - `/sys/devices/system/cpu/cpu{N}/cpufreq/scaling_max_freq` — maximum
//!   frequency in kHz
//! - `/sys/class/hwmon/hwmon{N}/` — hardware monitoring devices (temperatures)

use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// CPU Frequency
// ---------------------------------------------------------------------------

/// Per-core CPU frequency readings in MHz.
///
/// Both `per_core` and `max_per_core` have the same length — one entry per
/// logical CPU core.  Cores that couldn't be read get 0.
#[derive(Clone, Debug, Default)]
pub struct CpuFrequency {
    /// Current frequency per core in MHz
    pub per_core: Vec<u32>,
    /// Maximum frequency per core in MHz (used for percentage calculations)
    pub max_per_core: Vec<u32>,
}

impl CpuFrequency {
    /// Read current CPU frequencies from sysfs.
    ///
    /// Iterates through `/sys/devices/system/cpu/cpu0/`, `cpu1/`, etc.
    /// until a CPU index doesn't exist, reading both current and max
    /// frequencies.  Values are in kHz in sysfs and converted to MHz here.
    pub fn read() -> Self {
        let mut per_core = Vec::new();
        let mut max_per_core = Vec::new();

        // Try cpu0, cpu1, cpu2, ... until we find one that doesn't exist
        for cpu_idx in 0.. {
            let freq_path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq",
                cpu_idx
            );

            if !Path::new(&freq_path).exists() {
                break; // no more CPUs
            }

            per_core.push(Self::read_khz_as_mhz(&freq_path));

            let max_path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_max_freq",
                cpu_idx
            );
            max_per_core.push(Self::read_khz_as_mhz(&max_path));
        }

        Self { per_core, max_per_core }
    }

    /// Read a sysfs frequency file (value is in kHz) and return MHz.
    /// Returns 0 if the file can't be read or parsed.
    fn read_khz_as_mhz(path: &str) -> u32 {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(|khz| khz / 1000) // kHz -> MHz
            .unwrap_or_else(|| {
                tracing::debug!("Failed to read frequency from {}", path);
                0
            })
    }

    /// Calculate the average current frequency across all cores in MHz.
    ///
    /// Returns 0 if no cores were detected.
    pub fn average_mhz(&self) -> u32 {
        if self.per_core.is_empty() {
            return 0;
        }
        self.per_core.iter().sum::<u32>() / self.per_core.len() as u32
    }

    /// Get a single core's frequency as a percentage of its maximum.
    ///
    /// For example, if core 0 is running at 2000 MHz with a max of 4000 MHz,
    /// this returns 50.0.
    pub fn percentage(&self, core: usize) -> f32 {
        if core < self.per_core.len() && core < self.max_per_core.len() {
            let max = self.max_per_core[core];
            if max > 0 {
                return (self.per_core[core] as f32 / max as f32) * 100.0;
            }
        }
        0.0
    }

    /// Get the average frequency percentage across all cores.
    ///
    /// This is the metric used to drive the animation speed in frequency mode.
    pub fn average_percentage(&self) -> f32 {
        if self.per_core.is_empty() {
            return 0.0;
        }
        let sum: f32 = (0..self.per_core.len())
            .map(|i| self.percentage(i))
            .sum();
        sum / self.per_core.len() as f32
    }
}

// ---------------------------------------------------------------------------
// CPU Temperature
// ---------------------------------------------------------------------------

/// CPU temperature readings from hardware sensors.
///
/// Not all systems expose all fields — for example, AMD systems may only
/// have a `tctl`/`tdie` reading (stored as `package`) with no per-core
/// breakdown.
#[derive(Clone, Debug, Default)]
pub struct CpuTemperature {
    /// Per-core temperatures in degrees Celsius (if available)
    pub per_core: Vec<f32>,
    /// Package/die temperature in degrees Celsius (if available)
    pub package: Option<f32>,
    /// Critical temperature threshold — the CPU will throttle above this
    pub critical: Option<f32>,
}

impl CpuTemperature {
    /// Read CPU temperatures from the Linux hwmon subsystem.
    ///
    /// Scans `/sys/class/hwmon/hwmon*` for known CPU sensor drivers:
    /// - **`coretemp`** — Intel CPUs (package + per-core temps)
    /// - **`k10temp`** / **`zenpower`** — AMD CPUs (usually Tctl/Tdie only)
    /// - **`amdgpu`** — AMD APUs with integrated graphics
    /// - Anything starting with `cpu` — generic CPU sensor
    ///
    /// Returns the first sensor that provides temperature data.
    pub fn read() -> Self {
        let hwmon_base = Path::new("/sys/class/hwmon");
        if !hwmon_base.exists() {
            return Self::default();
        }

        let Ok(entries) = fs::read_dir(hwmon_base) else {
            return Self::default();
        };

        for entry in entries.flatten() {
            let hwmon_path = entry.path();
            let name_path = hwmon_path.join("name");

            // Read the sensor driver name (e.g. "coretemp", "k10temp")
            let Ok(name) = fs::read_to_string(&name_path) else {
                continue;
            };
            let name = name.trim();

            // Only read from known CPU temperature sensor drivers
            if matches!(name, "coretemp" | "k10temp" | "zenpower" | "amdgpu")
                || name.starts_with("cpu")
            {
                let result = Self::read_hwmon(&hwmon_path);
                if !result.per_core.is_empty() || result.package.is_some() {
                    return result; // found a working sensor
                }
            }
        }

        Self::default()
    }

    /// Read temperatures from a specific hwmon device directory.
    ///
    /// Hwmon exposes temperatures as files like:
    /// - `temp1_input` — temperature in millidegrees C (e.g. 45000 = 45.0C)
    /// - `temp1_label` — human-readable label (e.g. "Package id 0", "Core 0")
    /// - `temp1_crit`  — critical temperature threshold
    ///
    /// We classify readings by their label:
    /// - Labels containing "package", "tctl", or "tdie" -> package temperature
    /// - Labels containing "core" -> per-core temperatures
    /// - Unlabelled -> fallback to package temperature
    fn read_hwmon(hwmon_path: &Path) -> Self {
        let mut per_core = Vec::new();
        let mut package = None;
        let mut critical = None;

        let Ok(entries) = fs::read_dir(hwmon_path) else {
            return Self::default();
        };

        // Collect all temperature readings first, then classify them
        let mut temp_readings: Vec<(String, f32)> = Vec::new();

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();

            // Read temperature input files (e.g. temp1_input, temp2_input)
            if filename.starts_with("temp") && filename.ends_with("_input") {
                if let Some(celsius) = Self::read_millidegrees(&entry.path()) {
                    // Try to read the label file to identify what this sensor is
                    let label_file = filename.replace("_input", "_label");
                    let label = fs::read_to_string(hwmon_path.join(&label_file))
                        .map(|s| s.trim().to_lowercase())
                        .unwrap_or_default();
                    temp_readings.push((label, celsius));
                }
            }

            // Read critical temperature threshold (e.g. temp1_crit)
            if filename.starts_with("temp") && filename.ends_with("_crit") {
                if let Some(celsius) = Self::read_millidegrees(&entry.path()) {
                    critical = Some(celsius);
                }
            }
        }

        // Classify each reading based on its label
        for (label, temp) in temp_readings {
            if label.contains("package") || label.contains("tctl") || label.contains("tdie") {
                // Overall die/package temperature
                package = Some(temp);
            } else if label.contains("core") {
                // Individual CPU core temperature
                per_core.push(temp);
            } else if package.is_none() && per_core.is_empty() {
                // Unlabelled sensor with no other readings — treat as package temp
                package = Some(temp);
            }
        }

        Self { per_core, package, critical }
    }

    /// Read a millidegrees file and convert to Celsius.
    ///
    /// Linux hwmon reports temperatures in millidegrees Celsius (integer),
    /// e.g. `45000` means 45.0 C.  Returns `None` on read/parse failure.
    fn read_millidegrees(path: &Path) -> Option<f32> {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .map(|md| md as f32 / 1000.0)
            .or_else(|| {
                tracing::debug!("Failed to read temperature from {:?}", path);
                None
            })
    }

    /// Get the highest temperature reading across all sensors.
    ///
    /// Considers both per-core and package temperatures.
    /// Used to determine if the cat should sleep (temp below threshold)
    /// and to show the "Max:" row in the popup.
    pub fn max_temp(&self) -> f32 {
        let core_max = self.per_core.iter().copied().fold(0.0f32, f32::max);
        self.package.unwrap_or(0.0).max(core_max)
    }

    /// Get temperature as a percentage of the critical threshold.
    ///
    /// If no critical threshold is known, assumes 100 C.
    /// Used to drive the animation speed in temperature mode.
    pub fn percentage(&self) -> f32 {
        let max_temp = self.max_temp();
        let critical = self.critical.unwrap_or(100.0);
        ((max_temp / critical) * 100.0).clamp(0.0, 100.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_read() {
        let freq = CpuFrequency::read();
        // On a Linux system with cpufreq, we expect at least one core
        assert!(
            !freq.per_core.is_empty(),
            "Expected at least one CPU core frequency"
        );
        assert_eq!(
            freq.per_core.len(),
            freq.max_per_core.len(),
            "per_core and max_per_core should have same length"
        );

        // Every core should have a positive max frequency
        for &max in &freq.max_per_core {
            assert!(max > 0, "Max frequency should be positive, got {}", max);
        }
    }

    #[test]
    fn test_frequency_average_mhz() {
        let freq = CpuFrequency::read();
        let avg = freq.average_mhz();
        if !freq.per_core.is_empty() {
            assert!(avg > 0, "Average frequency should be positive on a running system");
        }
    }

    #[test]
    fn test_frequency_average_mhz_empty() {
        // Edge case: no cores detected
        let freq = CpuFrequency::default();
        assert_eq!(freq.average_mhz(), 0);
    }

    #[test]
    fn test_frequency_percentage() {
        // Test with known values instead of reading from the system
        let freq = CpuFrequency {
            per_core: vec![2000, 3000],
            max_per_core: vec![4000, 4000],
        };
        // Core 0: 2000/4000 = 50%, Core 1: 3000/4000 = 75%
        assert!((freq.percentage(0) - 50.0).abs() < 0.1);
        assert!((freq.percentage(1) - 75.0).abs() < 0.1);
        // Average: (50 + 75) / 2 = 62.5%
        assert!((freq.average_percentage() - 62.5).abs() < 0.1);
    }

    #[test]
    fn test_temperature_read() {
        let temp = CpuTemperature::read();
        // Temperature sensors may or may not be available (e.g. in containers),
        // but the call should never panic
        if !temp.per_core.is_empty() || temp.package.is_some() {
            let max = temp.max_temp();
            assert!(max > 0.0, "Max temp should be positive if sensors exist");
            assert!(max < 200.0, "Max temp should be reasonable, got {}", max);
        }
    }

    #[test]
    fn test_temperature_percentage() {
        let temp = CpuTemperature {
            per_core: vec![50.0, 60.0],
            package: Some(55.0),
            critical: Some(100.0),
        };
        // max_temp is 60.0 (highest core), percentage = 60/100 * 100 = 60%
        assert!((temp.percentage() - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_temperature_max_temp() {
        let temp = CpuTemperature {
            per_core: vec![50.0, 70.0],
            package: Some(65.0),
            critical: None,
        };
        // Max should be 70.0 (the highest per-core reading)
        assert!((temp.max_temp() - 70.0).abs() < f32::EPSILON);
    }
}
