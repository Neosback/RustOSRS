//! The loading screen: "OpenRune" fills up like a liquid as the first regions arrive, the waves
//! settle as it reaches the top, and the whole screen fades away. It is drawn over everything for
//! the first moments after a cache is opened and changes no state of the editor.
use super::EditorApp;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use web_time::Instant;

/// How long the screen takes to fade once everything is loaded.
const FADE_SECONDS: f32 = 0.7;
/// Give up waiting and fade anyway after this long (a cache with nothing near the camera).
const GIVE_UP_SECONDS: f32 = 45.0;
/// Slowest the level may rise, per second, so it never looks stuck.
const MIN_RATE: f32 = 0.02;

/// Progress and fade of the loading screen.
pub(super) struct Splash {
    active: bool,
    started: Instant,
    shown: f32,
    fade_started: Option<Instant>,
}

impl Default for Splash {
    fn default() -> Self {
        Self {
            active: false,
            started: Instant::now(),
            shown: 0.0,
            fade_started: None,
        }
    }
}

impl Splash {
    /// A cache is being opened: show the screen from the start.
    pub(super) fn begin(&mut self) {
        *self = Self {
            active: true,
            ..Self::default()
        };
    }

    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// Opening failed (the cache card explains it); hide the screen.
    pub(super) fn cancel(&mut self) {
        self.active = false;
    }

    /// Move toward `target` (0 to 1; at 1 the screen starts fading). Returns
    /// `(level, opacity)`, or `None` once the screen is gone.
    pub(super) fn step(&mut self, dt: f32, target: f32) -> Option<(f32, f32)> {
        if !self.active {
            return None;
        }
        let target = if self.started.elapsed().as_secs_f32() > GIVE_UP_SECONDS {
            1.0
        } else {
            target.clamp(0.0, 1.0)
        };
        // Ease toward the target, never backwards, never slower than MIN_RATE.
        let eased = (target - self.shown) * (1.0 - (-3.0 * dt).exp());
        if target > self.shown {
            self.shown = (self.shown + eased.max(MIN_RATE * dt)).min(target);
        }
        let mut opacity = 1.0;
        if target >= 1.0 && self.shown >= 0.995 {
            let began = *self.fade_started.get_or_insert_with(Instant::now);
            let faded = began.elapsed().as_secs_f32() / FADE_SECONDS;
            if faded >= 1.0 {
                self.active = false;
                return None;
            }
            opacity = 1.0 - faded;
        }
        Some((self.shown, opacity))
    }
}

/// How far loading is, from what the world reports. Opening the cache is a fifth of the bar; the
/// rest is the regions around the camera. Everything built and nothing still building is done.
pub(super) fn target(ready: bool, resident: usize, expected: usize, in_flight: usize) -> f32 {
    if !ready {
        return 0.12;
    }
    if resident > 0 && in_flight == 0 && resident * 2 >= expected {
        return 1.0;
    }
    0.2 + 0.78 * (resident as f32 / expected.max(1) as f32).min(1.0)
}

/// Draw the loading screen when it is active (call at the end of the frame).
pub(super) fn draw(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.ui.splash_active() {
        return;
    }
    let radius = app.settings.prefs.load_radius;
    let expected = usize::try_from((2 * radius + 1).pow(2)).unwrap_or(1);
    let goal = target(
        app.world.is_ready(),
        app.world.resident_regions(),
        expected,
        app.world.in_flight_count(),
    );
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
    let Some((level, opacity)) = app.ui.splash_step(dt, goal) else {
        return;
    };
    let rect = ctx.content_rect();
    let time = ctx.input(|i| i.time) as f32;
    egui::Area::new(egui::Id::new("loading_splash"))
        .order(egui::Order::Tooltip)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
            paint(ui, rect, level, time, opacity);
        });
    ctx.request_repaint();
}

fn paint(ui: &egui::Ui, rect: Rect, level: f32, time: f32, opacity: f32) {
    let painter = ui.painter();
    let background = Color32::from_rgb(12, 14, 20);
    painter.rect_filled(rect, 0.0, background.gamma_multiply(opacity));
    let size = (rect.width() * 0.13).clamp(48.0, 150.0);
    let galley = painter.layout_no_wrap(
        "OpenRune".to_owned(),
        FontId::proportional(size),
        Color32::PLACEHOLDER,
    );
    let text = Rect::from_center_size(rect.center() - Vec2::new(0.0, size * 0.15), galley.size());
    painter.galley(
        text.min,
        galley.clone(),
        Color32::from_rgb(70, 78, 96).gamma_multiply(opacity * 0.55),
    );
    // The liquid: the waves are tall while it rises and flatten as it nears the top.
    let calm = 1.0 - level.powi(3);
    let amplitude = (size * 0.07 * calm).max(0.6);
    let low = text.bottom() + amplitude * 2.0;
    let high = text.top() - amplitude * 2.0;
    let surface = low + (high - low) * level;
    let slice = 2.0_f32;
    let layers = [
        (Color32::from_rgb(40, 110, 190), 1.0_f32, 2.2_f32, 0.0_f32),
        (Color32::from_rgb(90, 190, 255), 0.75, -1.6, 1.7),
    ];
    for (colour, strength, speed, phase) in layers {
        let mut x = text.left();
        while x < text.right() {
            let mid = x + slice / 2.0;
            let wave = (mid * 0.045 + time * speed + phase).sin()
                + 0.5 * (mid * 0.021 - time * speed * 0.6 + phase).sin();
            let top = surface + wave * amplitude * strength;
            let clip = Rect::from_min_max(
                Pos2::new(x, top.min(text.bottom())),
                Pos2::new(x + slice, text.bottom() + 4.0),
            );
            painter.with_clip_rect(clip).galley(
                text.min,
                galley.clone(),
                colour.gamma_multiply(opacity),
            );
            x += slice;
        }
    }
    let label = format!("Opening the map  {:.0}%", level * 100.0);
    painter.text(
        Pos2::new(rect.center().x, text.bottom() + size * 0.55),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        Color32::from_rgb(150, 160, 180).gamma_multiply(opacity),
    );
    // A thin rule under the word, filled to the same level.
    let rule = Rect::from_center_size(
        Pos2::new(rect.center().x, text.bottom() + size * 0.3),
        Vec2::new(text.width(), 3.0),
    );
    painter.rect_filled(
        rule,
        1.5,
        Color32::from_rgb(40, 46, 60).gamma_multiply(opacity),
    );
    let filled = Rect::from_min_size(rule.min, Vec2::new(rule.width() * level, rule.height()));
    painter.rect_filled(
        filled,
        1.5,
        Color32::from_rgb(90, 190, 255).gamma_multiply(opacity),
    );
    painter.rect_stroke(
        rule,
        1.5,
        Stroke::new(0.0, Color32::TRANSPARENT),
        egui::StrokeKind::Outside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_never_goes_backwards_and_reaches_the_target() {
        let mut splash = Splash::default();
        splash.begin();
        let mut last = 0.0;
        for step in 0..600 {
            let goal = if step < 100 { 0.3 } else { 0.9 };
            let (level, _) = splash.step(0.016, goal).unwrap_or((last, 1.0));
            assert!(level >= last, "level fell at step {step}");
            last = level;
        }
        assert!((last - 0.9).abs() < 0.01, "ends at the target: {last}");
    }

    #[test]
    fn it_keeps_rising_slowly_even_when_the_target_barely_moves() {
        let mut splash = Splash::default();
        splash.begin();
        let before = splash.step(0.016, 0.5).map_or(0.0, |s| s.0);
        let mut after = before;
        for _ in 0..300 {
            after = splash.step(0.016, 0.5).map_or(after, |s| s.0);
        }
        assert!(after > before);
    }

    #[test]
    fn it_stays_until_loading_finishes_then_fades_out_and_goes() {
        let mut splash = Splash::default();
        splash.begin();
        for _ in 0..300 {
            let (_, opacity) = splash.step(0.016, 0.8).unwrap_or((0.0, 0.0));
            assert_eq!(opacity, 1.0);
        }
        // Loaded: the level fills, the fade starts, and the screen is gone after the fade.
        let mut gone = false;
        for _ in 0..400 {
            if splash.step(0.1, 1.0).is_none() {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert!(gone, "the splash disappears");
        assert!(splash.step(0.016, 1.0).is_none());
    }

    #[test]
    fn an_inactive_splash_draws_nothing() {
        let mut splash = Splash::default();
        assert!(splash.step(0.016, 0.5).is_none());
        splash.begin();
        splash.cancel();
        assert!(splash.step(0.016, 0.5).is_none());
    }

    #[test]
    fn the_target_follows_what_the_world_reports() {
        assert_eq!(target(false, 0, 9, 0), 0.12);
        assert!(target(true, 0, 9, 2) < 0.25);
        let half = target(true, 3, 9, 2);
        assert!(half > 0.4 && half < 0.8);
        assert_eq!(
            target(true, 5, 9, 0),
            1.0,
            "half built and nothing left to build"
        );
        assert!(target(true, 5, 9, 1) < 1.0, "still building");
        assert_eq!(target(true, 0, 9, 0), 0.2, "nothing resident yet");
    }
}
