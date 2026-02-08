/// About dialog overlay.
/// Shows logo, version, author info, and GitHub link.
/// Triggered by clicking the title bar label; closes on any click.
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::theme::{self, ThemeMode};
use super::Data;

/// PNG logos embedded at compile time.
const LOGO_PNG: &[u8] = include_bytes!("../../ivylace_logo.png");
const LOGO_DARK_PNG: &[u8] = include_bytes!("../../ivylace_logo_dark.png");

/// Noto Sans JP (kana subset) for Japanese text fallback.
const NOTO_SANS_JP_KANA: &[u8] = include_bytes!("../../assets/noto_sans_jp_kana.ttf");

/// About dialog — full-screen overlay with centered info panel.
pub(crate) struct AboutDialog {
    visible: Arc<AtomicBool>,
    glass_mode: Arc<AtomicBool>,
    /// Cached femtovg image ids (created on first draw): [glass, dark].
    logo_glass_id: Cell<Option<vg::ImageId>>,
    logo_dark_id: Cell<Option<vg::ImageId>>,
    /// Cached femtovg font ids (created on first draw): [latin, jp_kana].
    font_ids: Cell<Option<(vg::FontId, vg::FontId)>>,
    // Glass (light) logo
    logo_glass_width: u32,
    logo_glass_height: u32,
    logo_glass_rgba: Vec<u8>,
    // Dark logo
    logo_dark_width: u32,
    logo_dark_height: u32,
    logo_dark_rgba: Vec<u8>,
}

impl AboutDialog {
    pub fn new(
        cx: &mut Context,
        visible: Arc<AtomicBool>,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        // Decode PNGs at widget creation (not on every draw).
        let img_glass = image::load_from_memory(LOGO_PNG).expect("Failed to decode glass logo PNG");
        let rgba_glass = img_glass.to_rgba8();
        let (gw, gh) = rgba_glass.dimensions();
        let logo_glass_rgba = rgba_glass.into_raw();

        let img_dark = image::load_from_memory(LOGO_DARK_PNG).expect("Failed to decode dark logo PNG");
        let rgba_dark = img_dark.to_rgba8();
        let (dw, dh) = rgba_dark.dimensions();
        let logo_dark_rgba = rgba_dark.into_raw();

        let vis_for_hover = visible.clone();

        Self {
            visible,
            glass_mode,
            logo_glass_id: Cell::new(None),
            logo_dark_id: Cell::new(None),
            font_ids: Cell::new(None),
            logo_glass_width: gw,
            logo_glass_height: gh,
            logo_glass_rgba,
            logo_dark_width: dw,
            logo_dark_height: dh,
            logo_dark_rgba,
        }
        .build(cx, |_| {})
        .position_type(PositionType::SelfDirected)
        .left(Pixels(0.0))
        .top(Pixels(0.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        // Only intercept mouse events when dialog is visible;
        // when hidden, hoverable(false) lets events pass to widgets below.
        .hoverable(Data::params.map(move |_| vis_for_hover.load(Ordering::Relaxed)))
    }

    /// Ensure the logo texture is uploaded to the GPU (lazy init).
    /// Returns (image_id, width, height) for the current theme.
    fn ensure_logo(&self, canvas: &mut Canvas, mode: ThemeMode) -> Option<(vg::ImageId, u32, u32)> {
        let (id_cell, rgba, w, h) = match mode {
            ThemeMode::Glass => (&self.logo_glass_id, &self.logo_glass_rgba, self.logo_glass_width, self.logo_glass_height),
            ThemeMode::Dark => (&self.logo_dark_id, &self.logo_dark_rgba, self.logo_dark_width, self.logo_dark_height),
        };

        if let Some(id) = id_cell.get() {
            return Some((id, w, h));
        }

        // Reinterpret raw RGBA bytes as &[rgb::RGBA8]
        let pixel_count = w as usize * h as usize;
        let pixels: &[vg::rgb::RGBA8] = unsafe {
            std::slice::from_raw_parts(
                rgba.as_ptr() as *const vg::rgb::RGBA8,
                pixel_count,
            )
        };

        let img_ref = vg::imgref::Img::new(pixels, w as usize, h as usize);

        let id = canvas
            .create_image(img_ref, vg::ImageFlags::empty())
            .ok()?;
        id_cell.set(Some(id));
        Some((id, w, h))
    }

    /// Ensure fonts are loaded for femtovg text rendering (lazy init).
    /// Returns (latin_font_id, jp_kana_font_id) for fallback rendering.
    fn ensure_fonts(&self, canvas: &mut Canvas) -> Option<(vg::FontId, vg::FontId)> {
        if let Some(ids) = self.font_ids.get() {
            return Some(ids);
        }

        let latin_data = nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR;
        let latin_id = canvas.add_font_mem(latin_data).ok()?;
        let jp_id = canvas.add_font_mem(NOTO_SANS_JP_KANA).ok()?;
        self.font_ids.set(Some((latin_id, jp_id)));
        Some((latin_id, jp_id))
    }
}

impl View for AboutDialog {
    fn element(&self) -> Option<&'static str> {
        Some("about-dialog")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if !self.visible.load(Ordering::Relaxed) {
            return;
        }

        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) {
            ThemeMode::Glass
        } else {
            ThemeMode::Dark
        };
        let dpi = cx.scale_factor();

        // Get fonts for text rendering (latin + JP fallback)
        let font_ids = self.ensure_fonts(canvas);

        // ── Full-screen dimming overlay ──
        {
            let mut path = vg::Path::new();
            path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
            canvas.fill_path(&path, &vg::Paint::color(theme::about_overlay_bg(mode)));
        }

        // ── Centered panel ──
        let panel_w = 340.0 * dpi;
        let panel_h = 380.0 * dpi;
        let px = bounds.x + (bounds.w - panel_w) * 0.5;
        let py = bounds.y + (bounds.h - panel_h) * 0.5;
        let corner_r = 16.0 * dpi;

        // Panel background
        {
            let mut path = vg::Path::new();
            path.rounded_rect(px, py, panel_w, panel_h, corner_r);

            // Solid background for readability
            canvas.fill_path(&path, &vg::Paint::color(theme::about_panel_bg(mode)));

            // Border
            let mut border_paint = vg::Paint::color(theme::panel_border(mode));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);
        }

        // ── Logo image (theme-dependent) ──
        let logo_display_w = 260.0 * dpi;
        let logo_y = py + 24.0 * dpi;

        let logo_display_h = if let Some((logo_id, lw, lh)) = self.ensure_logo(canvas, mode) {
            let logo_aspect = lh as f32 / lw as f32;
            let h = logo_display_w * logo_aspect;
            let logo_x = px + (panel_w - logo_display_w) * 0.5;

            let paint = vg::Paint::image(
                logo_id,
                logo_x,
                logo_y,
                logo_display_w,
                h,
                0.0,
                1.0,
            );
            let mut path = vg::Path::new();
            path.rounded_rect(logo_x, logo_y, logo_display_w, h, 8.0 * dpi);
            canvas.fill_path(&path, &paint);
            h
        } else {
            // Fallback: use glass logo aspect ratio
            let logo_aspect = self.logo_glass_height as f32 / self.logo_glass_width as f32;
            logo_display_w * logo_aspect
        };

        // ── Text content ──
        let text_x = px + panel_w * 0.5;
        let mut text_y = logo_y + logo_display_h + 20.0 * dpi;

        // Helper for centered text
        let mut draw_text =
            |canvas: &mut Canvas, text: &str, size: f32, color: vg::Color, spacing: f32| {
                let mut paint = vg::Paint::color(color);
                paint.set_font_size(size * dpi);
                paint.set_text_align(vg::Align::Center);
                paint.set_text_baseline(vg::Baseline::Top);
                if let Some((latin, jp)) = font_ids {
                    paint.set_font(&[latin, jp]);
                }
                let _ = canvas.fill_text(text_x, text_y, text, &paint);
                text_y += (size + spacing) * dpi;
            };

        // "Ivylace"
        draw_text(canvas, "Ivylace", 22.0, theme::text_primary(mode), 6.0);

        // Version
        let version_text = concat!("v", env!("CARGO_PKG_VERSION"));
        draw_text(canvas, version_text, 12.0, theme::text_secondary(mode), 10.0);

        // Description
        draw_text(
            canvas,
            "Multiband Glue Compressor",
            11.0,
            theme::text_dim(mode),
            12.0,
        );

        // Author
        draw_text(
            canvas,
            "by きょーこ",
            12.0,
            theme::text_secondary(mode),
            16.0,
        );

        // GitHub link
        draw_text(
            canvas,
            "github.com/kyoko-hiebara/Ivylace",
            10.0,
            theme::about_link(mode),
            12.0,
        );

        // Built with
        draw_text(
            canvas,
            "Built with nih-plug",
            9.0,
            theme::text_dim(mode),
            0.0,
        );
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        if !self.visible.load(Ordering::Relaxed) {
            return;
        }

        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                self.visible.store(false, Ordering::Relaxed);
                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}
