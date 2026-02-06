//! CPU monitoring module
//!
//! Reads CPU usage percentages (both aggregate and per-core) using the
//! `systemstat` crate, which parses `/proc/stat` on Linux.
//!
//! ## Architecture
//!
//! The `systemstat` crate uses **blocking I/O** — it sleeps for a sample
//! interval, then reads `/proc/stat` again to calculate the delta.  Since
//! our COSMIC applet runs on an async event loop, we can't block it.
//!
//! Solution: a dedicated **OS thread** (`std::thread::spawn`) performs the
//! blocking reads, then pushes results through a **tokio `watch` channel**.
//! The applet's async `update()` method can read the latest value from the
//! channel without blocking.
//!
//! ## Error handling
//!
//! If a CPU read fails (e.g. `/proc/stat` is unavailable), we log the error
//! and send 0.0 / empty data rather than crashing.  This keeps the applet
//! alive even on unusual systems.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use systemstat::{Platform, System};
use tokio::sync::watch;

/// Snapshot of CPU usage at a point in time.
#[derive(Clone, Debug, Default)]
pub struct CpuUsage {
    /// Overall CPU usage across all cores (0.0 to 100.0)
    pub aggregate: f32,
    /// Usage for each individual core (0.0 to 100.0 each)
    pub per_core: Vec<f32>,
}

/// Monitors CPU usage on a background thread and provides the latest
/// readings via a `watch` channel.
///
/// ## Usage
///
/// ```ignore
/// let monitor = CpuMonitor::new();
/// monitor.start(Duration::from_millis(500));
///
/// // Later, on any thread:
/// let usage = monitor.current();
/// println!("CPU: {:.1}%", usage.aggregate);
/// ```
///
/// The monitor is automatically stopped when dropped.
pub struct CpuMonitor {
    /// Sender side of the watch channel (cloned into the background thread)
    tx: watch::Sender<CpuUsage>,
    /// Receiver side — call `.borrow()` to get the latest value
    rx: watch::Receiver<CpuUsage>,
    /// Shared flag to tell the background thread to stop
    stop_flag: Arc<AtomicBool>,
}

impl CpuMonitor {
    /// Create a new monitor.  Call `start()` to begin sampling.
    pub fn new() -> Self {
        // `watch::channel` holds exactly one value — the latest CPU reading.
        // Old values are automatically overwritten, so we never accumulate
        // a backlog.
        let (tx, rx) = watch::channel(CpuUsage::default());
        Self {
            tx,
            rx,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawn a background OS thread that samples CPU usage at the given
    /// interval and sends updates through the watch channel.
    pub fn start(&self, sample_interval: Duration) {
        let tx = self.tx.clone();
        let stop_flag = self.stop_flag.clone();

        thread::spawn(move || {
            let sys = System::new();

            while !stop_flag.load(Ordering::Relaxed) {
                // Start both measurements simultaneously so they cover
                // the same time window.  Each returns a "measurement in
                // progress" handle.
                let aggregate_measurement = sys.cpu_load_aggregate();
                let per_core_measurement = sys.cpu_load();

                // Sleep for the sample interval — systemstat needs this
                // time to measure the CPU usage delta between two reads
                // of /proc/stat.
                thread::sleep(sample_interval);

                // Finish the aggregate measurement: returns idle/user/system %
                let aggregate = aggregate_measurement
                    .and_then(|cpu| cpu.done())
                    .map(|load| (1.0 - load.idle) * 100.0)
                    .unwrap_or_else(|e| {
                        tracing::error!("CPU aggregate measurement error: {}", e);
                        0.0 // fallback: report 0% rather than crashing
                    });

                // Finish the per-core measurement
                let per_core = per_core_measurement
                    .and_then(|cpus| cpus.done())
                    .map(|loads| {
                        loads.iter().map(|l| (1.0 - l.idle) * 100.0).collect()
                    })
                    .unwrap_or_else(|e| {
                        tracing::error!("CPU per-core measurement error: {}", e);
                        Vec::new() // fallback: empty list
                    });

                // Send the new data — any number of readers can see it
                if let Err(e) = tx.send(CpuUsage { aggregate, per_core }) {
                    tracing::error!("Failed to send CPU update: {}", e);
                }
            }
        });
    }

    /// Signal the background thread to stop.  It will exit on the next
    /// iteration of its sampling loop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Get the latest CPU usage snapshot.  This never blocks — it just
    /// reads the most recent value that the background thread has sent.
    pub fn current(&self) -> CpuUsage {
        self.rx.borrow().clone()
    }
}

impl Default for CpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Automatically stop the monitoring thread when the monitor is dropped,
/// preventing leaked threads.
impl Drop for CpuMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
