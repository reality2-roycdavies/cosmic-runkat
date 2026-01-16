//! D-Bus client for communicating with the cosmic-runkat daemon
//!
//! Provides a proxy interface for the tray and settings app to
//! communicate with the daemon.

use zbus::{proxy, Connection, Result};

use crate::daemon::{OBJECT_PATH, SERVICE_NAME};

/// D-Bus proxy for the RunkatDaemon
#[proxy(
    interface = "org.cosmicrunkat.Service1",
    default_service = "org.cosmicrunkat.Service1",
    default_path = "/org/cosmicrunkat/Service1"
)]
trait RunkatService {
    /// Get the current CPU usage percentage
    fn get_cpu_percent(&self) -> Result<f32>;

    /// Get the sleep threshold percentage
    fn get_sleep_threshold(&self) -> Result<f32>;

    /// Set the sleep threshold percentage
    fn set_sleep_threshold(&self, threshold: f32) -> Result<()>;

    /// Get the maximum FPS
    fn get_max_fps(&self) -> Result<f32>;

    /// Set the maximum FPS
    fn set_max_fps(&self, fps: f32) -> Result<()>;

    /// Get the minimum FPS
    fn get_min_fps(&self) -> Result<f32>;

    /// Set the minimum FPS
    fn set_min_fps(&self, fps: f32) -> Result<()>;

    /// Calculate the current animation FPS based on CPU usage
    fn get_animation_fps(&self) -> Result<f32>;

    /// Check if the cat should be sleeping
    fn is_sleeping(&self) -> Result<bool>;

    /// Get whether to show percentage on icon
    fn get_show_percentage(&self) -> Result<bool>;

    /// Set whether to show percentage on icon
    fn set_show_percentage(&self, show: bool) -> Result<()>;
}

/// Client for the cosmic-runkat daemon
pub struct RunkatClient<'a> {
    proxy: RunkatServiceProxy<'a>,
}

impl<'a> RunkatClient<'a> {
    /// Connect to the daemon
    pub async fn connect(conn: &'a Connection) -> Result<Self> {
        let proxy = RunkatServiceProxy::new(conn).await?;
        Ok(Self { proxy })
    }

    /// Get the current CPU usage percentage
    pub async fn get_cpu_percent(&self) -> Result<f32> {
        self.proxy.get_cpu_percent().await
    }

    /// Get the sleep threshold
    pub async fn get_sleep_threshold(&self) -> Result<f32> {
        self.proxy.get_sleep_threshold().await
    }

    /// Set the sleep threshold
    pub async fn set_sleep_threshold(&self, threshold: f32) -> Result<()> {
        self.proxy.set_sleep_threshold(threshold).await
    }

    /// Get the maximum FPS
    pub async fn get_max_fps(&self) -> Result<f32> {
        self.proxy.get_max_fps().await
    }

    /// Set the maximum FPS
    pub async fn set_max_fps(&self, fps: f32) -> Result<()> {
        self.proxy.set_max_fps(fps).await
    }

    /// Get the minimum FPS
    pub async fn get_min_fps(&self) -> Result<f32> {
        self.proxy.get_min_fps().await
    }

    /// Set the minimum FPS
    pub async fn set_min_fps(&self, fps: f32) -> Result<()> {
        self.proxy.set_min_fps(fps).await
    }

    /// Get the current animation FPS
    pub async fn get_animation_fps(&self) -> Result<f32> {
        self.proxy.get_animation_fps().await
    }

    /// Check if the cat is sleeping
    pub async fn is_sleeping(&self) -> Result<bool> {
        self.proxy.is_sleeping().await
    }

    /// Get whether to show percentage on icon
    pub async fn get_show_percentage(&self) -> Result<bool> {
        self.proxy.get_show_percentage().await
    }

    /// Set whether to show percentage on icon
    pub async fn set_show_percentage(&self, show: bool) -> Result<()> {
        self.proxy.set_show_percentage(show).await
    }
}

/// Create a new D-Bus connection
pub async fn create_connection() -> Result<Connection> {
    Connection::session().await
}
