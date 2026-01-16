//! D-Bus Daemon for cosmic-runkat
//!
//! Provides a central service for CPU monitoring and configuration management.
//! Both the tray and settings app connect to this daemon via D-Bus.

use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::watch;
use zbus::{interface, ConnectionBuilder};

use crate::config::Config;
use crate::cpu::CpuMonitor;

/// D-Bus service name
pub const SERVICE_NAME: &str = "org.cosmicrunkat.Service1";

/// D-Bus object path
pub const OBJECT_PATH: &str = "/org/cosmicrunkat/Service1";

/// The daemon service that monitors CPU and manages configuration
pub struct RunkatDaemon {
    config: Arc<RwLock<Config>>,
    cpu_percent: Arc<RwLock<f32>>,
}

impl RunkatDaemon {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            cpu_percent: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Update the CPU percentage (called by the monitor thread)
    pub fn update_cpu(&self, percent: f32) {
        if let Ok(mut cpu) = self.cpu_percent.write() {
            *cpu = percent;
        }
    }
}

#[interface(name = "org.cosmicrunkat.Service1")]
impl RunkatDaemon {
    /// Get the current CPU usage percentage
    fn get_cpu_percent(&self) -> f32 {
        self.cpu_percent.read().map(|v| *v).unwrap_or(0.0)
    }

    /// Get the sleep threshold percentage
    fn get_sleep_threshold(&self) -> f32 {
        self.config.read().map(|c| c.sleep_threshold).unwrap_or(5.0)
    }

    /// Set the sleep threshold percentage
    fn set_sleep_threshold(&self, threshold: f32) -> zbus::fdo::Result<()> {
        if let Ok(mut config) = self.config.write() {
            config.sleep_threshold = threshold.clamp(0.0, 50.0);
            config.save().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get the maximum FPS
    fn get_max_fps(&self) -> f32 {
        self.config.read().map(|c| c.max_fps).unwrap_or(10.0)
    }

    /// Set the maximum FPS
    fn set_max_fps(&self, fps: f32) -> zbus::fdo::Result<()> {
        if let Ok(mut config) = self.config.write() {
            config.max_fps = fps.clamp(1.0, 30.0);
            config.save().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get the minimum FPS
    fn get_min_fps(&self) -> f32 {
        self.config.read().map(|c| c.min_fps).unwrap_or(1.0)
    }

    /// Set the minimum FPS
    fn set_min_fps(&self, fps: f32) -> zbus::fdo::Result<()> {
        if let Ok(mut config) = self.config.write() {
            config.min_fps = fps.clamp(0.5, 10.0);
            config.save().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }

    /// Calculate the current animation FPS based on CPU usage
    fn get_animation_fps(&self) -> f32 {
        let cpu = self.cpu_percent.read().map(|v| *v).unwrap_or(0.0);
        self.config
            .read()
            .map(|c| c.calculate_fps(cpu))
            .unwrap_or(0.0)
    }

    /// Check if the cat should be sleeping (CPU below threshold)
    fn is_sleeping(&self) -> bool {
        let cpu = self.cpu_percent.read().map(|v| *v).unwrap_or(0.0);
        let threshold = self.config.read().map(|c| c.sleep_threshold).unwrap_or(5.0);
        cpu < threshold
    }

    /// Get whether to show percentage on icon
    fn get_show_percentage(&self) -> bool {
        self.config.read().map(|c| c.show_percentage).unwrap_or(true)
    }

    /// Set whether to show percentage on icon
    fn set_show_percentage(&self, show: bool) -> zbus::fdo::Result<()> {
        if let Ok(mut config) = self.config.write() {
            config.show_percentage = show;
            config.save().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }
}

/// Run the daemon
pub async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();
    let daemon = RunkatDaemon::new(config);
    let cpu_percent = daemon.cpu_percent.clone();

    // Start CPU monitoring
    let cpu_monitor = CpuMonitor::new();
    let mut cpu_rx = cpu_monitor.subscribe();
    cpu_monitor.start(Duration::from_millis(500));

    // Spawn task to update daemon's CPU value
    let cpu_update_handle = tokio::spawn(async move {
        while cpu_rx.changed().await.is_ok() {
            let percent = *cpu_rx.borrow();
            if let Ok(mut cpu) = cpu_percent.write() {
                *cpu = percent;
            }
        }
    });

    // Register D-Bus service
    let _conn = ConnectionBuilder::session()?
        .name(SERVICE_NAME)?
        .serve_at(OBJECT_PATH, daemon)?
        .build()
        .await?;

    println!("cosmic-runkat daemon running on D-Bus: {}", SERVICE_NAME);

    // Keep running until interrupted
    tokio::signal::ctrl_c().await?;
    println!("Shutting down daemon...");

    cpu_update_handle.abort();
    Ok(())
}
