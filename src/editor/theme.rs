/// Theme constants for the Ivylace Compressor GUI.
/// Supports Dark and Glass (macOS Tahoe-inspired) themes.
use nih_plug_vizia::vizia::prelude::Color;
use nih_plug_vizia::vizia::vg;

// ── Theme mode ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Glass,
}

// ── Band colors ──────────────────────────────────────────────

pub fn band_colors(mode: ThemeMode) -> [vg::Color; 4] {
    match mode {
        ThemeMode::Dark => [
            vg::Color::rgb(0xFF, 0x6B, 0x6B), // Low:     red
            vg::Color::rgb(0xFF, 0xA9, 0x4D), // LowMid:  orange
            vg::Color::rgb(0x69, 0xDB, 0x7C), // HighMid: green
            vg::Color::rgb(0x74, 0xC0, 0xFC), // High:    blue
        ],
        ThemeMode::Glass => [
            vg::Color::rgb(0xE0, 0x45, 0x45), // Low:     deeper red
            vg::Color::rgb(0xE0, 0x8A, 0x30), // LowMid:  deeper orange
            vg::Color::rgb(0x38, 0xA8, 0x52), // HighMid: deeper green
            vg::Color::rgb(0x3A, 0x8F, 0xD4), // High:    deeper blue
        ],
    }
}

pub const BAND_NAMES: [&str; 4] = ["Low", "LowMid", "HighMid", "High"];

// ── Accent / special ─────────────────────────────────────────

pub fn accent() -> vg::Color {
    vg::Color::rgb(0xE8, 0xA8, 0x38) // accent gold
}
pub fn solo_color() -> vg::Color {
    vg::Color::rgb(0xFF, 0xD4, 0x3B) // Yellow
}

// ── Page background ──────────────────────────────────────────

pub fn page_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x0A, 0x0A, 0x1A),
        ThemeMode::Glass => vg::Color::rgb(0xE8, 0xF0, 0xFA),
    }
}

pub fn page_bg_top(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x0A, 0x0A, 0x1A),
        ThemeMode::Glass => vg::Color::rgb(0xB8, 0xD8, 0xF0), // Sky blue
    }
}

pub fn page_bg_bottom(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x0A, 0x0A, 0x1A),
        ThemeMode::Glass => vg::Color::rgb(0xE8, 0xF0, 0xFA), // Near white
    }
}

// ── Glass panel ──────────────────────────────────────────────

pub fn glass_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 18),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 140),
    }
}
pub fn glass_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 30),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 200),
    }
}

// ── Title bar ────────────────────────────────────────────────

pub fn title_bar_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 15),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 100),
    }
}
pub fn title_bar_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 12),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 160),
    }
}
pub fn title_bar_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 100),
        ThemeMode::Glass => vg::Color::rgba(40, 50, 70, 160),
    }
}

// ── Header ───────────────────────────────────────────────────

pub fn header_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 18),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 120),
    }
}
pub fn header_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 15),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 180),
    }
}

// ── Panel ────────────────────────────────────────────────────

pub fn panel_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 10),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 100),
    }
}
pub fn panel_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 20),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 160),
    }
}
pub fn divider(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 20),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 20),
    }
}

// ── Knob ─────────────────────────────────────────────────────

pub fn knob_track(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 20),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 25),
    }
}
pub fn knob_body_light(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 45),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 200),
    }
}
pub fn knob_body_dark(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 38),
        ThemeMode::Glass => vg::Color::rgba(200, 215, 235, 160),
    }
}
pub fn knob_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 38),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 220),
    }
}

// ── Text ─────────────────────────────────────────────────────

pub fn text_primary(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 230),
        ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 230),
    }
}
pub fn text_secondary(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 128),
        ThemeMode::Glass => vg::Color::rgba(50, 60, 85, 160),
    }
}
pub fn text_dim(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 77),
        ThemeMode::Glass => vg::Color::rgba(80, 90, 110, 120),
    }
}
pub fn text_on_active(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 216),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 216),
    }
}

// ── Segmented control ────────────────────────────────────────

pub fn seg_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 15),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 60),
    }
}
pub fn seg_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 20),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 140),
    }
}
pub fn seg_active_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 38),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 180),
    }
}
pub fn seg_active_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 242),
        ThemeMode::Glass => vg::Color::rgba(20, 30, 50, 240),
    }
}
pub fn seg_inactive_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 89),
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 140),
    }
}

// ── Toggle ───────────────────────────────────────────────────

pub fn toggle_off_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 15),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 80),
    }
}
pub fn toggle_off_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 25),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 160),
    }
}
pub fn toggle_off_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 128),
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 180),
    }
}
pub fn toggle_power_active(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 240),
        ThemeMode::Glass => vg::Color::rgba(20, 30, 50, 230),
    }
}
pub fn toggle_power_inactive(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 140),
        ThemeMode::Glass => vg::Color::rgba(80, 90, 110, 120),
    }
}

// ── Meter ────────────────────────────────────────────────────

pub fn meter_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 76),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 30),
    }
}
pub fn meter_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 20),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 140),
    }
}
pub fn meter_seg_line(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 102),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 40),
    }
}
pub fn meter_fill_blue(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x4A, 0x9E, 0xFF),
        ThemeMode::Glass => vg::Color::rgb(0x3A, 0x80, 0xE0),
    }
}
pub fn meter_fill_red(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0xFF, 0x44, 0x66),
        ThemeMode::Glass => vg::Color::rgb(0xE0, 0x30, 0x50),
    }
}
pub fn meter_fill_blue_light(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x66, 0xBB, 0xFF),
        ThemeMode::Glass => vg::Color::rgb(0x50, 0xA0, 0xE8),
    }
}
pub fn meter_peak(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0xFF, 0x6B, 0x6B),
        ThemeMode::Glass => vg::Color::rgb(0xE0, 0x45, 0x45),
    }
}
pub fn meter_scale(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 51),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 50),
    }
}
pub fn meter_readout_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 204),
        ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 200),
    }
}
pub fn meter_readout_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 51),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 40),
    }
}
pub fn meter_gr_label(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 77),
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 130),
    }
}
pub fn meter_shadow(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 80),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 30),
    }
}

// ── Crossover display ────────────────────────────────────────

pub fn xover_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 51),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 40),
    }
}
pub fn xover_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 13),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 120),
    }
}
pub fn xover_grid(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 10),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 15),
    }
}
pub fn xover_grid_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 51),
        ThemeMode::Glass => vg::Color::rgba(40, 50, 70, 120),
    }
}
pub fn xover_handle_label_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 153),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 200),
    }
}

// ── Saturation section ───────────────────────────────────────

pub fn sat_section_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 8),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 50),
    }
}
pub fn sat_section_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 15),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 130),
    }
}
pub fn sat_label(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 77),
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 130),
    }
}

// ── Spectrum ─────────────────────────────────────────────────

pub fn spectrum_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 40),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 30),
    }
}
pub fn spectrum_fill_top(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(100, 180, 255, 25),
        ThemeMode::Glass => vg::Color::rgba(60, 130, 220, 40),
    }
}
pub fn spectrum_fill_bottom(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(100, 180, 255, 5),
        ThemeMode::Glass => vg::Color::rgba(60, 130, 220, 8),
    }
}
pub fn spectrum_stroke(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(120, 200, 255, 100),
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 120),
    }
}
pub fn spectrum_top_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 10),
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 80),
    }
}

// ── About dialog ────────────────────────────────────────────

pub fn about_overlay_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 180),
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 100),
    }
}
pub fn about_link(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(120, 200, 255, 200),
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 200),
    }
}

// ── Helpers ──────────────────────────────────────────────────

/// Create a color from a band color with modified alpha
pub fn with_alpha(c: vg::Color, alpha: f32) -> vg::Color {
    vg::Color::rgbaf(c.r, c.g, c.b, alpha)
}

/// Get the band color for a given band index
pub fn band_color(band_idx: usize, mode: ThemeMode) -> vg::Color {
    band_colors(mode)[band_idx.min(3)]
}

/// Convert a femtovg Color to a VIZIA Color
pub fn to_vizia(c: vg::Color) -> Color {
    Color::rgba(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    )
}
