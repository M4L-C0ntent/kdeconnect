//! Reads the COSMIC desktop theme (accent colour, dark/light mode) directly from
//! the host filesystem. `dirs::config_dir()` resolves to the Flatpak sandbox
//! when running as a Flatpak, so we always use `dirs::home_dir()` instead.

use serde::Deserialize;

/// COSMIC's default teal, used as a fallback when the host theme cannot be read.
pub const FALLBACK_TEAL: cosmic::iced::Color = cosmic::iced::Color {
    r: 0.067,
    g: 0.533,
    b: 0.533,
    a: 1.0,
};

#[derive(Deserialize)]
struct SrgbaColor {
    red: f32,
    green: f32,
    blue: f32,
}

#[derive(Deserialize)]
struct AccentFile {
    base: SrgbaColor,
}

/// Reads the user's current COSMIC accent colour from the host config directory.
///
/// Returns `None` if any file is missing or cannot be parsed, in which case
/// the caller should fall back to [`FALLBACK_TEAL`].
pub fn try_load_cosmic_accent() -> Option<cosmic::iced::Color> {
    let home = dirs::home_dir()?;
    let cosmic_cfg = home.join(".config").join("cosmic");

    // Read dark/light preference; default to dark when the file is absent.
    let is_dark = std::fs::read_to_string(
        cosmic_cfg
            .join("com.system76.CosmicTheme.Mode")
            .join("v1")
            .join("is_dark"),
    )
    .map(|s| s.trim() == "true")
    .unwrap_or(true);

    let theme_dir = if is_dark {
        "CosmicTheme.Dark"
    } else {
        "CosmicTheme.Light"
    };

    let accent_path = cosmic_cfg
        .join(format!("com.system76.{theme_dir}"))
        .join("v1")
        .join("accent");

    let text = std::fs::read_to_string(accent_path).ok()?;
    let parsed: AccentFile = ron::from_str(&text).ok()?;

    Some(cosmic::iced::Color {
        r: parsed.base.red,
        g: parsed.base.green,
        b: parsed.base.blue,
        a: 1.0,
    })
}
