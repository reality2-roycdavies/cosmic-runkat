//! System information module for CPU frequency and temperature
//!
//! Reads per-core frequency from sysfs and temperature from hwmon.

use std::fs;
use std::path::Path;

/// Per-core CPU frequency in MHz
#[derive(Clone, Debug, Default)]
pub struct CpuFrequency {
    /// Current frequency per core in MHz
    pub per_core: Vec<u32>,
    /// Maximum frequency per core in MHz (for percentage calculation)
    pub max_per_core: Vec<u32>,
}

impl CpuFrequency {
    /// Read current CPU frequencies from sysfs
    pub fn read() -> Self {
        let mut per_core = Vec::new();
        let mut max_per_core = Vec::new();

        let mut cpu_idx = 0;
        loop {
            let freq_path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq",
                cpu_idx
            );
            let max_path = format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_max_freq",
                cpu_idx
            );

            if !Path::new(&freq_path).exists() {
                break;
            }

            // Read current frequency (in kHz, convert to MHz)
            if let Ok(content) = fs::read_to_string(&freq_path) {
                if let Ok(khz) = content.trim().parse::<u32>() {
                    per_core.push(khz / 1000);
                } else {
                    per_core.push(0);
                }
            } else {
                per_core.push(0);
            }

            // Read max frequency
            if let Ok(content) = fs::read_to_string(&max_path) {
                if let Ok(khz) = content.trim().parse::<u32>() {
                    max_per_core.push(khz / 1000);
                } else {
                    max_per_core.push(0);
                }
            } else {
                max_per_core.push(0);
            }

            cpu_idx += 1;
        }

        Self { per_core, max_per_core }
    }

    /// Get frequency as percentage of max for a given core
    pub fn percentage(&self, core: usize) -> f32 {
        if core < self.per_core.len() && core < self.max_per_core.len() {
            let max = self.max_per_core[core];
            if max > 0 {
                return (self.per_core[core] as f32 / max as f32) * 100.0;
            }
        }
        0.0
    }

    /// Get average frequency percentage across all cores
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

/// CPU temperature readings
#[derive(Clone, Debug, Default)]
pub struct CpuTemperature {
    /// Per-core temperatures in Celsius (if available)
    pub per_core: Vec<f32>,
    /// Package/die temperature in Celsius (if available)
    pub package: Option<f32>,
    /// Critical temperature threshold (if available)
    pub critical: Option<f32>,
}

impl CpuTemperature {
    /// Read CPU temperatures from hwmon
    pub fn read() -> Self {
        let mut result = Self::default();

        // Find the CPU hwmon device
        let hwmon_base = Path::new("/sys/class/hwmon");
        if !hwmon_base.exists() {
            return result;
        }

        let Ok(entries) = fs::read_dir(hwmon_base) else {
            return result;
        };

        for entry in entries.flatten() {
            let hwmon_path = entry.path();

            // Check if this is a CPU temperature sensor
            let name_path = hwmon_path.join("name");
            if let Ok(name) = fs::read_to_string(&name_path) {
                let name = name.trim();
                // Common CPU temp sensor names
                if name == "coretemp" || name == "k10temp" || name == "zenpower"
                    || name == "amdgpu" || name.starts_with("cpu")
                {
                    result = Self::read_hwmon(&hwmon_path);
                    if !result.per_core.is_empty() || result.package.is_some() {
                        break;
                    }
                }
            }
        }

        result
    }

    /// Read temperatures from a specific hwmon device
    fn read_hwmon(hwmon_path: &Path) -> Self {
        let mut per_core = Vec::new();
        let mut package = None;
        let mut critical = None;

        // Read all temp*_input files
        let Ok(entries) = fs::read_dir(hwmon_path) else {
            return Self::default();
        };

        let mut temp_readings: Vec<(String, f32)> = Vec::new();

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.starts_with("temp") && filename.ends_with("_input") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(millidegrees) = content.trim().parse::<i32>() {
                        let celsius = millidegrees as f32 / 1000.0;

                        // Get the label to determine if this is package or core
                        let label_file = filename.replace("_input", "_label");
                        let label_path = hwmon_path.join(&label_file);
                        let label = fs::read_to_string(&label_path)
                            .map(|s| s.trim().to_lowercase())
                            .unwrap_or_default();

                        temp_readings.push((label, celsius));
                    }
                }
            }

            // Also look for critical temp
            if filename.starts_with("temp") && filename.ends_with("_crit") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(millidegrees) = content.trim().parse::<i32>() {
                        critical = Some(millidegrees as f32 / 1000.0);
                    }
                }
            }
        }

        // Sort and categorize readings
        for (label, temp) in temp_readings {
            if label.contains("package") || label.contains("tctl") || label.contains("tdie") {
                package = Some(temp);
            } else if label.contains("core") {
                per_core.push(temp);
            } else if package.is_none() && per_core.is_empty() {
                // Fallback: first reading becomes package temp
                package = Some(temp);
            }
        }

        Self { per_core, package, critical }
    }

    /// Get the maximum temperature (for animation driving)
    pub fn max_temp(&self) -> f32 {
        let core_max = self.per_core.iter().cloned().fold(0.0f32, f32::max);
        self.package.unwrap_or(0.0).max(core_max)
    }

    /// Get temperature as percentage of critical (or assumed 100°C max)
    pub fn percentage(&self) -> f32 {
        let max_temp = self.max_temp();
        let critical = self.critical.unwrap_or(100.0);
        ((max_temp / critical) * 100.0).clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_read() {
        let freq = CpuFrequency::read();
        // Should have at least one core on any system
        println!("Frequencies: {:?}", freq.per_core);
        println!("Max frequencies: {:?}", freq.max_per_core);
    }

    #[test]
    fn test_temperature_read() {
        let temp = CpuTemperature::read();
        println!("Per-core temps: {:?}", temp.per_core);
        println!("Package temp: {:?}", temp.package);
        println!("Critical: {:?}", temp.critical);
    }
}
