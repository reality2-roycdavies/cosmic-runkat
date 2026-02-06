//! CPU monitoring module
//!
//! Uses systemstat crate to read CPU usage percentage.
//! Tracks both aggregate CPU usage and per-core usage.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use systemstat::{Platform, System};
use tokio::sync::watch;

/// CPU usage data including aggregate and per-core percentages
#[derive(Clone, Debug, Default)]
pub struct CpuUsage {
    /// Aggregate CPU usage percentage (0-100)
    pub aggregate: f32,
    /// Per-core CPU usage percentages (0-100 each)
    pub per_core: Vec<f32>,
}

/// CPU monitor that periodically samples CPU usage
pub struct CpuMonitor {
    /// Sender for CPU percentage updates (aggregate only, for compatibility)
    tx: watch::Sender<f32>,
    /// Receiver for CPU percentage updates (can be cloned)
    rx: watch::Receiver<f32>,
    /// Sender for full CPU usage data (aggregate + per-core)
    full_tx: watch::Sender<CpuUsage>,
    /// Receiver for full CPU usage data
    full_rx: watch::Receiver<CpuUsage>,
    /// Flag to stop the monitoring thread
    stop_flag: Arc<AtomicBool>,
}

impl CpuMonitor {
    /// Create a new CPU monitor
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(0.0);
        let (full_tx, full_rx) = watch::channel(CpuUsage::default());
        Self { tx, rx, full_tx, full_rx, stop_flag: Arc::new(AtomicBool::new(false)) }
    }

    /// Start the monitoring thread
    ///
    /// The thread samples CPU usage at the specified interval and sends
    /// updates through the watch channel.
    pub fn start(&self, sample_interval: Duration) {
        let tx = self.tx.clone();
        let full_tx = self.full_tx.clone();
        let stop_flag = self.stop_flag.clone();

        thread::spawn(move || {
            let sys = System::new();

            while !stop_flag.load(Ordering::Relaxed) {
                // Start both aggregate and per-core measurements
                let aggregate_measurement = sys.cpu_load_aggregate();
                let per_core_measurement = sys.cpu_load();

                // Wait for sample interval
                thread::sleep(sample_interval);

                // Get aggregate result
                let aggregate = match aggregate_measurement {
                    Ok(cpu) => match cpu.done() {
                        Ok(cpu_load) => (1.0 - cpu_load.idle) * 100.0,
                        Err(e) => {
                            tracing::error!("CPU aggregate measurement error: {}", e);
                            0.0
                        }
                    },
                    Err(e) => {
                        tracing::error!("CPU aggregate load error: {}", e);
                        0.0
                    }
                };

                // Get per-core results
                let per_core = match per_core_measurement {
                    Ok(cpus) => match cpus.done() {
                        Ok(cpu_loads) => cpu_loads
                            .iter()
                            .map(|load| (1.0 - load.idle) * 100.0)
                            .collect(),
                        Err(e) => {
                            tracing::error!("CPU per-core measurement error: {}", e);
                            Vec::new()
                        }
                    },
                    Err(e) => {
                        tracing::error!("CPU per-core load error: {}", e);
                        Vec::new()
                    }
                };

                // Send aggregate (for backward compatibility)
                if let Err(e) = tx.send(aggregate) {
                    tracing::error!("Failed to send CPU update: {}", e);
                }

                // Send full data
                let usage = CpuUsage { aggregate, per_core };
                if let Err(e) = full_tx.send(usage) {
                    tracing::error!("Failed to send full CPU update: {}", e);
                }
            }
        });
    }

    /// Stop the monitoring thread
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Get the current CPU percentage (last sampled value)
    #[allow(dead_code)] // Still useful for non-async code paths
    pub fn current(&self) -> f32 {
        *self.rx.borrow()
    }

    /// Get the current full CPU usage data (aggregate + per-core)
    #[allow(dead_code)]
    pub fn current_full(&self) -> CpuUsage {
        self.full_rx.borrow().clone()
    }

}

impl Default for CpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CpuMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
