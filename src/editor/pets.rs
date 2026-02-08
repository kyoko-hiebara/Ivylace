/// Walking pets widget — a chick and a cat stroll across the bottom of the band strips.
/// Pure cosmetic easter egg. Uses Instant-based animation in View::draw().
/// Drawn with femtovg paths (not emoji/text) for cross-platform compatibility.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::theme::ThemeMode;

const PET_HEIGHT: f32 = 20.0;

/// Chick speed (pixels per second)
const CHICK_SPEED: f32 = 18.0;
/// Cat speed (pixels per second)
const CAT_SPEED: f32 = 24.0;

/// Walking pets overlay — draws at the very bottom of the band strip area.
pub struct WalkingPets {
    glass_mode: Arc<AtomicBool>,
    start_time: Instant,
}

impl WalkingPets {
    pub fn new(cx: &mut Context, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        Self {
            glass_mode,
            start_time: Instant::now(),
        }
        .build(cx, |_| {})
        .height(Pixels(PET_HEIGHT))
        .width(Stretch(1.0))
        .hoverable(false)
    }
}

impl View for WalkingPets {
    fn element(&self) -> Option<&'static str> {
        Some("walking-pets")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 20.0 || bounds.h < 4.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let w = bounds.w;
        let ground_y = bounds.y + bounds.h - 2.0 * dpi;

        // ── Chick (walks left → right) ──
        {
            let chick_x = bounds.x + (elapsed * CHICK_SPEED * dpi) % (w + 30.0 * dpi) - 15.0 * dpi;
            let frame = ((elapsed * 3.5) as u32) % 2;
            draw_chick(canvas, chick_x, ground_y, dpi, frame, mode);
        }

        // ── Cat (walks right → left) ──
        {
            let cat_raw = (elapsed * CAT_SPEED * dpi) % (w + 30.0 * dpi);
            let cat_x = bounds.x + w - cat_raw + 15.0 * dpi;
            let frame = ((elapsed * 3.0) as u32) % 2;
            draw_cat(canvas, cat_x, ground_y, dpi, frame, mode);
        }
    }
}

/// Draw a tiny chick (facing right) at (x, ground_y) using femtovg paths.
/// `frame` alternates legs for walk animation.
fn draw_chick(canvas: &mut Canvas, x: f32, ground_y: f32, dpi: f32, frame: u32, mode: ThemeMode) {
    let s = dpi * 1.0; // scale

    let body_color = match mode {
        ThemeMode::Dark => vg::Color::rgba(255, 220, 80, 200),  // warm yellow
        ThemeMode::Glass => vg::Color::rgba(230, 190, 50, 180),
    };
    let beak_color = vg::Color::rgba(255, 140, 40, 220); // orange
    let eye_color = match mode {
        ThemeMode::Dark => vg::Color::rgba(60, 30, 20, 230),
        ThemeMode::Glass => vg::Color::rgba(40, 20, 10, 220),
    };
    let leg_color = vg::Color::rgba(220, 140, 40, 180);

    // Body (oval)
    {
        let mut path = vg::Path::new();
        path.ellipse(x, ground_y - 6.0 * s, 5.0 * s, 4.5 * s);
        canvas.fill_path(&path, &vg::Paint::color(body_color));
    }

    // Head (circle, slightly up-right)
    {
        let mut path = vg::Path::new();
        path.circle(x + 3.5 * s, ground_y - 10.0 * s, 3.0 * s);
        canvas.fill_path(&path, &vg::Paint::color(body_color));
    }

    // Eye
    {
        let mut path = vg::Path::new();
        path.circle(x + 4.5 * s, ground_y - 11.0 * s, 0.7 * s);
        canvas.fill_path(&path, &vg::Paint::color(eye_color));
    }

    // Beak (small triangle pointing right)
    {
        let mut path = vg::Path::new();
        path.move_to(x + 6.0 * s, ground_y - 10.0 * s);
        path.line_to(x + 8.5 * s, ground_y - 9.5 * s);
        path.line_to(x + 6.0 * s, ground_y - 8.8 * s);
        path.close();
        canvas.fill_path(&path, &vg::Paint::color(beak_color));
    }

    // Legs (alternating for walk)
    {
        let mut paint = vg::Paint::color(leg_color);
        paint.set_line_width(1.0 * s);
        paint.set_line_cap(vg::LineCap::Round);

        let (leg1_dx, leg2_dx) = if frame == 0 { (-1.5, 1.5) } else { (1.0, -1.0) };

        // Left leg
        let mut path = vg::Path::new();
        path.move_to(x - 1.0 * s, ground_y - 2.0 * s);
        path.line_to(x + leg1_dx * s, ground_y);
        canvas.stroke_path(&path, &paint);

        // Right leg
        let mut path = vg::Path::new();
        path.move_to(x + 1.5 * s, ground_y - 2.0 * s);
        path.line_to(x + 1.5 * s + leg2_dx * s, ground_y);
        canvas.stroke_path(&path, &paint);
    }
}

/// Draw a tiny cat (facing left) at (x, ground_y) using femtovg paths.
/// `frame` alternates legs for walk animation.
fn draw_cat(canvas: &mut Canvas, x: f32, ground_y: f32, dpi: f32, frame: u32, mode: ThemeMode) {
    let s = dpi * 1.0;

    let body_color = match mode {
        ThemeMode::Dark => vg::Color::rgba(180, 160, 200, 190),  // lavender
        ThemeMode::Glass => vg::Color::rgba(140, 130, 160, 170),
    };
    let ear_color = match mode {
        ThemeMode::Dark => vg::Color::rgba(200, 170, 220, 200),
        ThemeMode::Glass => vg::Color::rgba(160, 140, 180, 180),
    };
    let eye_color = match mode {
        ThemeMode::Dark => vg::Color::rgba(100, 200, 120, 230),  // green eyes
        ThemeMode::Glass => vg::Color::rgba(60, 160, 80, 220),
    };
    let nose_color = vg::Color::rgba(220, 140, 140, 200); // pink
    let leg_color = match mode {
        ThemeMode::Dark => vg::Color::rgba(160, 140, 180, 180),
        ThemeMode::Glass => vg::Color::rgba(120, 110, 140, 160),
    };
    let tail_color = body_color;

    // Body (horizontal oval)
    {
        let mut path = vg::Path::new();
        path.ellipse(x, ground_y - 6.0 * s, 7.0 * s, 4.0 * s);
        canvas.fill_path(&path, &vg::Paint::color(body_color));
    }

    // Head (circle, to the left since cat faces left)
    {
        let mut path = vg::Path::new();
        path.circle(x - 5.5 * s, ground_y - 9.0 * s, 3.5 * s);
        canvas.fill_path(&path, &vg::Paint::color(body_color));
    }

    // Ears (two triangles)
    {
        // Left ear
        let mut path = vg::Path::new();
        path.move_to(x - 7.5 * s, ground_y - 11.5 * s);
        path.line_to(x - 8.5 * s, ground_y - 15.0 * s);
        path.line_to(x - 5.5 * s, ground_y - 12.0 * s);
        path.close();
        canvas.fill_path(&path, &vg::Paint::color(ear_color));

        // Right ear
        let mut path = vg::Path::new();
        path.move_to(x - 4.0 * s, ground_y - 12.0 * s);
        path.line_to(x - 3.0 * s, ground_y - 15.5 * s);
        path.line_to(x - 2.0 * s, ground_y - 11.0 * s);
        path.close();
        canvas.fill_path(&path, &vg::Paint::color(ear_color));
    }

    // Eyes (two small circles)
    {
        let mut path = vg::Path::new();
        path.circle(x - 6.5 * s, ground_y - 9.5 * s, 0.8 * s);
        canvas.fill_path(&path, &vg::Paint::color(eye_color));

        let mut path = vg::Path::new();
        path.circle(x - 4.2 * s, ground_y - 9.5 * s, 0.8 * s);
        canvas.fill_path(&path, &vg::Paint::color(eye_color));
    }

    // Nose (tiny triangle)
    {
        let mut path = vg::Path::new();
        path.move_to(x - 5.5 * s, ground_y - 8.5 * s);
        path.line_to(x - 5.0 * s, ground_y - 7.8 * s);
        path.line_to(x - 6.0 * s, ground_y - 7.8 * s);
        path.close();
        canvas.fill_path(&path, &vg::Paint::color(nose_color));
    }

    // Legs (4 legs, alternating pairs for walk)
    {
        let mut paint = vg::Paint::color(leg_color);
        paint.set_line_width(1.2 * s);
        paint.set_line_cap(vg::LineCap::Round);

        let (front_dy, back_dy) = if frame == 0 { (0.0, 1.5) } else { (1.5, 0.0) };

        // Front legs
        let mut path = vg::Path::new();
        path.move_to(x - 3.0 * s, ground_y - 3.0 * s);
        path.line_to(x - 3.5 * s - front_dy * s, ground_y);
        canvas.stroke_path(&path, &paint);

        let mut path = vg::Path::new();
        path.move_to(x - 1.5 * s, ground_y - 3.0 * s);
        path.line_to(x - 1.0 * s + front_dy * s, ground_y);
        canvas.stroke_path(&path, &paint);

        // Back legs
        let mut path = vg::Path::new();
        path.move_to(x + 3.0 * s, ground_y - 3.0 * s);
        path.line_to(x + 2.5 * s - back_dy * s, ground_y);
        canvas.stroke_path(&path, &paint);

        let mut path = vg::Path::new();
        path.move_to(x + 4.5 * s, ground_y - 3.0 * s);
        path.line_to(x + 5.0 * s + back_dy * s, ground_y);
        canvas.stroke_path(&path, &paint);
    }

    // Tail (curved line from back)
    {
        let tail_wave = (elapsed_wave(frame) * 0.3).sin() * 2.0;
        let mut path = vg::Path::new();
        path.move_to(x + 6.5 * s, ground_y - 6.0 * s);
        path.bezier_to(
            x + 9.0 * s, ground_y - 10.0 * s + tail_wave * s,
            x + 11.0 * s, ground_y - 12.0 * s,
            x + 10.0 * s, ground_y - 14.0 * s + tail_wave * s,
        );
        let mut paint = vg::Paint::color(tail_color);
        paint.set_line_width(1.5 * s);
        paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&path, &paint);
    }
}

/// Simple wave value for tail animation
#[inline]
fn elapsed_wave(frame: u32) -> f32 {
    frame as f32 * std::f32::consts::PI
}
