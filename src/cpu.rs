//! CPU monitoring module
//!
//! Uses systemstat crate to read CPU usage percentage.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use systemstat::{Platform, System};
use tokio::sync::watch;

/// CPU monitor that periodically samples CPU usage
pub struct CpuMonitor {
    /// Sender for CPU percentage updates
    tx: watch::Sender<f32>,
    /// Receiver for CPU percentage updates (can be cloned)
    rx: watch::Receiver<f32>,
    /// Flag to stop the monitoring thread
    stop_flag: Arc<AtomicBool>,
}

impl CpuMonitor {
    /// Create a new CPU monitor
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(0.0);
        Self {
            tx,
            rx,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the monitoring thread
    ///
    /// The thread samples CPU usage at the specified interval and sends
    /// updates through the watch channel.
    pub fn start(&self, sample_interval: Duration) {
        let tx = self.tx.clone();
        let stop_flag = self.stop_flag.clone();

        thread::spawn(move || {
            let sys = System::new();

            while !stop_flag.load(Ordering::Relaxed) {
                // Start CPU measurement
                match sys.cpu_load_aggregate() {
                    Ok(cpu) => {
                        // Wait for sample interval
                        thread::sleep(sample_interval);

                        // Get the result
                        match cpu.done() {
                            Ok(cpu_load) => {
                                // Calculate total CPU usage (everything except idle)
                                let usage = (1.0 - cpu_load.idle) * 100.0;
                                if let Err(e) = tx.send(usage) {
                                    eprintln!("Failed to send CPU update: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("CPU measurement error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("CPU load error: {}", e);
                        thread::sleep(sample_interval);
                    }
                }
            }
        });
    }

    /// Stop the monitoring thread
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Get the current CPU percentage (last sampled value)
    pub fn current(&self) -> f32 {
        *self.rx.borrow()
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
