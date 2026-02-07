/// Theme constants for the Ivylace Compressor GUI.
/// Dark theme inspired by the Figma glassmorphism design.
use nih_plug_vizia::vizia::vg;

// ── Band colors ──────────────────────────────────────────────
pub fn band_colors() -> [vg::Color; 4] {
    [
        vg::Color::rgb(0xFF, 0x6B, 0x6B), // Low:     red
        vg::Color::rgb(0xFF, 0xA9, 0x4D), // LowMid:  orange
        vg::Color::rgb(0x69, 0xDB, 0x7C), // HighMid: green
        vg::Color::rgb(0x74, 0xC0, 0xFC), // High:    blue
    ]
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
pub fn page_bg() -> vg::Color {
    vg::Color::rgb(0x0A, 0x0A, 0x1A)
}

// ── Glass panel ──────────────────────────────────────────────
pub fn glass_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 18)
}
pub fn glass_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 30)
}

// ── Title bar ────────────────────────────────────────────────
pub fn title_bar_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 15)
}
pub fn title_bar_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 12)
}

// ── Header ───────────────────────────────────────────────────
pub fn header_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 18)
}
pub fn header_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 15)
}

// ── Panel ────────────────────────────────────────────────────
pub fn panel_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 10)
}
pub fn panel_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 20)
}
pub fn divider() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 20)
}

// ── Knob ─────────────────────────────────────────────────────
pub fn knob_track() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 20)
}
pub fn knob_body_light() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 45)
}
pub fn knob_body_dark() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 38)
}
pub fn knob_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 38)
}

// ── Text ─────────────────────────────────────────────────────
pub fn text_primary() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 230)
}
pub fn text_secondary() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 128)
}
pub fn text_dim() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 77)
}
pub fn text_on_active() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 216)
}

// ── Segmented control ────────────────────────────────────────
pub fn seg_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 15)
}
pub fn seg_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 20)
}
pub fn seg_active_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 38)
}
pub fn seg_active_text() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 242)
}
pub fn seg_inactive_text() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 89)
}

// ── Toggle ───────────────────────────────────────────────────
pub fn toggle_off_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 15)
}
pub fn toggle_off_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 25)
}
pub fn toggle_off_text() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 128)
}

// ── Meter ────────────────────────────────────────────────────
pub fn meter_bg() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 76)
}
pub fn meter_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 20)
}
pub fn meter_seg_line() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 102)
}
pub fn meter_fill_blue() -> vg::Color {
    vg::Color::rgb(0x4A, 0x9E, 0xFF)
}
pub fn meter_fill_red() -> vg::Color {
    vg::Color::rgb(0xFF, 0x44, 0x66)
}
pub fn meter_peak() -> vg::Color {
    vg::Color::rgb(0xFF, 0x6B, 0x6B)
}
pub fn meter_scale() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 51)
}
pub fn meter_readout_text() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 204)
}
pub fn meter_readout_bg() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 51)
}
pub fn meter_gr_label() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 77)
}

// ── Crossover display ────────────────────────────────────────
pub fn xover_bg() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 51)
}
pub fn xover_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 13)
}
pub fn xover_grid() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 10)
}
pub fn xover_grid_text() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 51)
}
pub fn xover_handle_label_bg() -> vg::Color {
    vg::Color::rgba(0, 0, 0, 153)
}

// ── Saturation section ───────────────────────────────────────
pub fn sat_section_bg() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 8)
}
pub fn sat_section_border() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 15)
}
pub fn sat_label() -> vg::Color {
    vg::Color::rgba(255, 255, 255, 77)
}

// ── Helpers ──────────────────────────────────────────────────

/// Create a color from a band color with modified alpha
pub fn with_alpha(c: vg::Color, alpha: f32) -> vg::Color {
    vg::Color::rgbaf(c.r, c.g, c.b, alpha)
}

/// Get the band color for a given band index
pub fn band_color(band_idx: usize) -> vg::Color {
    band_colors()[band_idx.min(3)]
}
