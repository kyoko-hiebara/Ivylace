/// Theme constants for the Ivylace Compressor GUI.
/// Dark: purple glassy aesthetic (#cc7eb1 / #663f58)
/// Glass: light theme with #89c3eb tint
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
        ThemeMode::Dark => vg::Color::rgb(0x4D, 0x43, 0x98), // #4d4398
        ThemeMode::Glass => vg::Color::rgb(0xE8, 0xF0, 0xFA),
    }
}

pub fn page_bg_top(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x2B, 0x26, 0x52), // dark purple top
        ThemeMode::Glass => vg::Color::rgb(0xF2, 0xF6, 0xFB), // near-white with subtle blue tint
    }
}

pub fn page_bg_bottom(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x1E, 0x1A, 0x40), // deeper dark purple bottom
        ThemeMode::Glass => vg::Color::rgb(0xFA, 0xFB, 0xFD), // almost pure white
    }
}

// ── Glass panel ──────────────────────────────────────────────

pub fn glass_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 30),  // brighter purple tint
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 40), // #89c3eb tint
    }
}
pub fn glass_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 50),  // brighter purple border
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 60), // #89c3eb border
    }
}

// ── Title bar ────────────────────────────────────────────────

pub fn title_bar_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 22),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 30), // #89c3eb tint
    }
}
pub fn title_bar_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 30),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 45), // #89c3eb border
    }
}
pub fn title_bar_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 140),  // pure white
        ThemeMode::Glass => vg::Color::rgba(40, 50, 70, 160),
    }
}

// ── Header ───────────────────────────────────────────────────

pub fn header_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 28),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 35), // #89c3eb tint
    }
}
pub fn header_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 35),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50), // #89c3eb border
    }
}

// ── Panel ────────────────────────────────────────────────────

pub fn panel_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 22),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 30), // #89c3eb tint
    }
}
pub fn panel_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 40),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50), // #89c3eb border
    }
}
pub fn divider(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 22),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50), // #89c3eb
    }
}

// ── Knob ─────────────────────────────────────────────────────

pub fn knob_track(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 22),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50), // #89c3eb
    }
}
pub fn knob_body_light(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 65),  // brighter purple glass
        ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 160),
    }
}
pub fn knob_body_dark(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(102, 63, 88, 80),    // brighter deep purple
        ThemeMode::Glass => vg::Color::rgba(110, 165, 210, 120), // #89c3eb darker shade
    }
}
pub fn knob_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 55),  // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 70), // #89c3eb border
    }
}

// ── Text ─────────────────────────────────────────────────────

pub fn text_primary(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 230),  // pure white
        ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 230),
    }
}
pub fn text_secondary(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 150),  // pure white
        ThemeMode::Glass => vg::Color::rgba(50, 60, 85, 160),
    }
}
pub fn text_dim(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 170),  // brighter for visibility
        ThemeMode::Glass => vg::Color::rgba(40, 55, 80, 190),    // darker for visibility
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
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 30),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 25),  // #89c3eb tint
    }
}
pub fn seg_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 40),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50),  // #89c3eb border
    }
}
pub fn seg_active_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 75),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 70),  // #89c3eb active
    }
}
pub fn seg_active_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 242),  // pure white
        ThemeMode::Glass => vg::Color::rgba(20, 30, 50, 240),
    }
}
pub fn seg_inactive_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 110),  // pure white
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 140),
    }
}

// ── Toggle ───────────────────────────────────────────────────

pub fn toggle_off_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 30),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 25),  // #89c3eb tint
    }
}
pub fn toggle_off_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 45),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 55),  // #89c3eb border
    }
}
pub fn toggle_off_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 180),  // pure white
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 180),
    }
}
pub fn toggle_power_active(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 240),  // pure white
        ThemeMode::Glass => vg::Color::rgba(20, 30, 50, 230),
    }
}
pub fn toggle_power_inactive(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 160),  // pure white
        ThemeMode::Glass => vg::Color::rgba(80, 90, 110, 120),
    }
}

// ── Meter ────────────────────────────────────────────────────

pub fn meter_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(102, 63, 88, 80),     // deep purple well
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 30),  // #89c3eb tint
    }
}
pub fn meter_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 25),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 60),  // #89c3eb border
    }
}
pub fn meter_seg_line(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(102, 63, 88, 100),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50),  // #89c3eb
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
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 65),   // pure white
        ThemeMode::Glass => vg::Color::rgba(40, 60, 90, 50),
    }
}
pub fn meter_readout_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 204),  // pure white
        ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 200),
    }
}
pub fn meter_readout_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(102, 63, 88, 50),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 25),  // #89c3eb tint
    }
}
pub fn meter_gr_label(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 100),  // pure white
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 130),
    }
}
pub fn meter_shadow(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(40, 35, 80, 120),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 30),  // #89c3eb
    }
}

// ── Analog GR meter ──────────────────────────────────────────

pub fn analog_meter_arc(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 40),   // purple tint arc
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50),  // #89c3eb arc
    }
}
pub fn analog_meter_tick(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 80),
        ThemeMode::Glass => vg::Color::rgba(40, 60, 90, 70),
    }
}
pub fn analog_meter_tick_label(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 55),
        ThemeMode::Glass => vg::Color::rgba(40, 60, 90, 50),
    }
}
pub fn analog_meter_pivot(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 140),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 120),
    }
}

// ── Crossover display ────────────────────────────────────────

pub fn xover_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(102, 63, 88, 50),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 25),  // #89c3eb tint
    }
}
pub fn xover_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 18),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 50),  // #89c3eb border
    }
}
pub fn xover_grid(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 12),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 30),  // #89c3eb
    }
}
pub fn xover_grid_text(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 65),   // pure white
        ThemeMode::Glass => vg::Color::rgba(40, 50, 70, 120),
    }
}
pub fn xover_handle_label_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(55, 48, 110, 220),    // #4d4398-based pill bg
        ThemeMode::Glass => vg::Color::rgba(200, 230, 250, 220), // light #89c3eb pill bg
    }
}

// ── Saturation section ───────────────────────────────────────

pub fn sat_section_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 20),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 18),  // #89c3eb subtle tint
    }
}
pub fn sat_section_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 32),   // brighter
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 45),  // #89c3eb border
    }
}
pub fn sat_label(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 100),  // pure white
        ThemeMode::Glass => vg::Color::rgba(60, 70, 90, 130),
    }
}

// ── Spectrum ─────────────────────────────────────────────────

pub fn spectrum_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(102, 63, 88, 40),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 20),  // #89c3eb tint
    }
}
pub fn spectrum_fill_top(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 140, 200, 30),   // purple spectrum fill
        ThemeMode::Glass => vg::Color::rgba(60, 130, 220, 40),
    }
}
pub fn spectrum_fill_bottom(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 140, 200, 6),
        ThemeMode::Glass => vg::Color::rgba(60, 130, 220, 8),
    }
}
pub fn spectrum_stroke(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(220, 160, 210, 110),  // lavender stroke
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 120),
    }
}
pub fn spectrum_top_border(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(204, 126, 177, 14),
        ThemeMode::Glass => vg::Color::rgba(137, 195, 235, 40),  // #89c3eb border
    }
}

// ── Spectrum delta (cut / boost) ────────────────────────────

/// Delta cut fill (gain reduction, below 0dB line) — top of gradient
pub fn spectrum_delta_cut_top(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(100, 160, 255, 50),   // blue tint
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 55),
    }
}
/// Delta cut fill — bottom of gradient
pub fn spectrum_delta_cut_bottom(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(100, 160, 255, 10),
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 10),
    }
}
/// Delta cut stroke
pub fn spectrum_delta_cut_stroke(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(120, 180, 255, 140),
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 150),
    }
}
/// Delta boost fill (makeup gain, above 0dB line) — top of gradient
pub fn spectrum_delta_boost_top(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 130, 80, 50),    // orange tint
        ThemeMode::Glass => vg::Color::rgba(220, 100, 50, 55),
    }
}
/// Delta boost fill — bottom of gradient
pub fn spectrum_delta_boost_bottom(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 130, 80, 10),
        ThemeMode::Glass => vg::Color::rgba(220, 100, 50, 10),
    }
}
/// Delta boost stroke
pub fn spectrum_delta_boost_stroke(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 150, 100, 140),
        ThemeMode::Glass => vg::Color::rgba(220, 100, 50, 150),
    }
}
/// 0dB reference line
pub fn spectrum_zero_line(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 30),
        ThemeMode::Glass => vg::Color::rgba(40, 50, 70, 35),
    }
}

// ── Delta monitor ───────────────────────────────────────────

/// Active color for the delta monitor toggle (cyan/teal to stand out)
pub fn delta_monitor_active(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgb(0x00, 0xCC, 0xCC),   // cyan/teal
        ThemeMode::Glass => vg::Color::rgb(0x00, 0xA0, 0xA0),  // deeper teal
    }
}

// ── About dialog ────────────────────────────────────────────

pub fn about_overlay_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(40, 35, 80, 200),     // #4d4398-based dimming
        ThemeMode::Glass => vg::Color::rgba(0, 0, 0, 100),
    }
}
pub fn about_panel_bg(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(60, 52, 120, 240),    // #4d4398-based panel
        ThemeMode::Glass => vg::Color::rgba(230, 242, 252, 240), // light #89c3eb panel
    }
}
pub fn about_link(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(160, 200, 255, 200),  // light blue link
        ThemeMode::Glass => vg::Color::rgba(40, 120, 200, 200),
    }
}

// ── Theme toggle icon ───────────────────────────────────────

pub fn theme_toggle_sun(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 220, 100, 220),  // warm yellow sun
        ThemeMode::Glass => vg::Color::rgba(255, 220, 100, 220),
    }
}
pub fn theme_toggle_sun_ray(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 220, 100, 160),
        ThemeMode::Glass => vg::Color::rgba(255, 220, 100, 160),
    }
}
pub fn theme_toggle_moon(mode: ThemeMode) -> vg::Color {
    match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 200),  // pure white moon
        ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 200),
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
