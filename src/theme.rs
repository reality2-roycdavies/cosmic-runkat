//! Theme integration module
//!
//! Provides abstraction over COSMIC theme detection.
//! Uses manual RON parsing with graceful fallback to defaults.

/// Theme colors for icon rendering
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    /// Foreground color (for icon sprites)
    pub foreground: (u8, u8, u8),
    /// Whether the theme is dark mode
    pub is_dark: bool,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self { foreground: (200, 200, 200), is_dark: true }
    }
}

/// Get current theme colors from COSMIC
///
/// Tries manual RON file parsing with fallback to defaults.
pub fn get_cosmic_theme_colors() -> ThemeColors {
    match try_manual_theme_parsing() {
        Ok(colors) => colors,
        Err(e) => {
            tracing::warn!("Failed to load COSMIC theme: {}, using defaults", e);
            ThemeColors::default()
        }
    }
}

/// Path to the COSMIC theme config directory (~/.config/cosmic)
fn cosmic_config_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Ok(dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(".config/cosmic"))
}

/// Parse theme colors from COSMIC config files on disk
fn try_manual_theme_parsing() -> Result<ThemeColors, Box<dyn std::error::Error>> {
    let config_dir = cosmic_config_dir()?;

    // Check if dark mode is enabled
    let theme_path = config_dir.join("com.system76.CosmicTheme.Mode/v1/is_dark");
    let is_dark = std::fs::read_to_string(theme_path)
        .map(|s| s.trim() == "true")
        .unwrap_or(true);

    // Read the foreground color from the appropriate theme (Dark or Light)
    let mode = if is_dark { "Dark" } else { "Light" };
    let background_path = config_dir
        .join(format!("com.system76.CosmicTheme.{}/v1", mode))
        .join("background");
    let content = std::fs::read_to_string(background_path)?;
    let foreground = parse_color_from_ron(&content, "on")
        .ok_or("Failed to parse foreground color from theme")?;

    tracing::debug!(
        "Loaded COSMIC theme: RGB({}, {}, {}), dark={}",
        foreground.0,
        foreground.1,
        foreground.2,
        is_dark
    );

    Ok(ThemeColors { foreground, is_dark })
}

/// Parse a color from COSMIC theme RON format
fn parse_color_from_ron(content: &str, color_name: &str) -> Option<(u8, u8, u8)> {
    // Basic parser for COSMIC theme RON
    // Looks for `color_name: ( red: X, green: Y, blue: Z ... )`

    let key = format!("{}:", color_name);
    let rest = content.split(&key).nth(1)?;

    let start = rest.find('(')?;
    let end = rest[start..].find(')')?;
    let block = &rest[start + 1..start + end];

    let extract = |name: &str| -> Option<f32> {
        let name_key = format!("{}:", name);
        let val_part = block.split(&name_key).nth(1)?;
        let val_str = val_part.split(',').next()?.trim();
        val_str.parse().ok()
    };

    let r = extract("red")?;
    let g = extract("green")?;
    let b = extract("blue")?;

    Some((
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_from_ron() {
        let ron_content = r#"
        (
            base: (red: 0.1, green: 0.2, blue: 0.3, alpha: 1.0),
            on: (red: 0.784, green: 0.784, blue: 0.784, alpha: 1.0),
        )
        "#;

        let color = parse_color_from_ron(ron_content, "on");
        assert!(color.is_some());

        let (r, g, b) = color.unwrap();
        assert_eq!(r, 199); // 0.784 * 255 ≈ 199
        assert_eq!(g, 199);
        assert_eq!(b, 199);
    }

    #[test]
    fn test_theme_colors_default() {
        let theme = ThemeColors::default();
        assert_eq!(theme.foreground, (200, 200, 200));
        assert_eq!(theme.is_dark, true);
    }
}
