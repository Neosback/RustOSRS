//! Minimap status orbs: Hitpoints, Prayer, Run, and Special attack.

use super::EditorApp;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

mod ids {
    pub const ORB_FRAME: u32 = 1071;
    pub const HP_ORB: u32 = 1060;
    pub const HP_ICON: u32 = 1067;
    pub const PRAYER_ORB: u32 = 1063;
    pub const PRAYER_ACTIVE: u32 = 1066;
    pub const PRAYER_ICON: u32 = 1068;
    pub const PRAYER_ICON_ACTIVE: u32 = 1058;
    pub const RUN_ORB: u32 = 1064;
    pub const RUN_ACTIVE: u32 = 1065;
    pub const RUN_ICON: u32 = 1070;
    pub const WALK_ICON: u32 = 1069;
    pub const SPEC_ORB: u32 = 1607;
    pub const SPEC_ICON: u32 = 1610;
}

pub(super) fn draw(app: &mut EditorApp, ui: &egui::Ui, rect: Rect, scale: f32) {
    let (minimap_origin, orb_origin) = if app.settings.prefs.show_minimap {
        let frame_w = 210.0 * scale;
        let origin = Pos2::new(rect.right() - frame_w - 10.0, rect.top() + 10.0);
        (Some(origin), Pos2::new(origin.x, origin.y + 36.0 * scale))
    } else {
        (None, Pos2::new(rect.right() - 56.0, rect.top() + 16.0))
    };

    let orb_spacing = 38.0 * scale;
    let orb_radius = 13.5 * scale;
    let painter = ui.painter().with_clip_rect(rect);

    let player = &app.settings.prefs.player;
    let hp = player.hitpoints;
    let max_hp = u16::from(player.levels[3]).max(1);
    let prayer = player.prayer;
    let max_prayer = u16::from(player.levels[5]).max(1);
    let run_energy = player.run_energy;
    let running = player.running;

    // 1. Hitpoints Orb
    let hp_pos = if minimap_origin.is_some() {
        Pos2::new(orb_origin.x - 24.0 * scale, orb_origin.y)
    } else {
        orb_origin
    };
    render_single_orb(
        app,
        ui,
        &painter,
        hp_pos,
        orb_radius,
        ids::HP_ORB,
        ids::HP_ICON,
        hp as f32 / max_hp as f32,
        Color32::from_rgb(180, 20, 20),
        &format!("{hp}"),
        "Hitpoints",
        format!("Hitpoints: {hp} / {max_hp}"),
        None,
    );

    // 2. Prayer Orb
    let prayer_pos = if minimap_origin.is_some() {
        Pos2::new(orb_origin.x - 24.0 * scale, orb_origin.y + orb_spacing)
    } else {
        Pos2::new(orb_origin.x, orb_origin.y + orb_spacing)
    };
    let (prayer_orb, prayer_icon) = if prayer > 0 {
        (ids::PRAYER_ACTIVE, ids::PRAYER_ICON_ACTIVE)
    } else {
        (ids::PRAYER_ORB, ids::PRAYER_ICON)
    };
    render_single_orb(
        app,
        ui,
        &painter,
        prayer_pos,
        orb_radius,
        prayer_orb,
        prayer_icon,
        prayer as f32 / max_prayer as f32,
        Color32::from_rgb(40, 140, 210),
        &format!("{prayer}"),
        "Prayer",
        format!("Prayer: {prayer} / {max_prayer}"),
        None,
    );

    // 3. Run Orb
    let run_pos = if minimap_origin.is_some() {
        Pos2::new(
            orb_origin.x - 16.0 * scale,
            orb_origin.y + orb_spacing * 2.0,
        )
    } else {
        Pos2::new(orb_origin.x, orb_origin.y + orb_spacing * 2.0)
    };
    let (run_orb, run_icon) = if running {
        (ids::RUN_ACTIVE, ids::RUN_ICON)
    } else {
        (ids::RUN_ORB, ids::WALK_ICON)
    };
    let toggle_run = render_single_orb(
        app,
        ui,
        &painter,
        run_pos,
        orb_radius,
        run_orb,
        run_icon,
        run_energy as f32 / 100.0,
        Color32::from_rgb(200, 160, 20),
        &format!("{run_energy}"),
        "Run",
        format!(
            "Run Energy: {run_energy}% ({})",
            if running { "Running" } else { "Walking" }
        ),
        Some(egui::Id::new("gameframe_run_toggle")),
    );
    if toggle_run {
        app.settings.prefs.player.running = !app.settings.prefs.player.running;
    }

    // 4. Special Attack Orb
    let spec_pos = Pos2::new(
        if minimap_origin.is_some() {
            orb_origin.x + 4.0 * scale
        } else {
            orb_origin.x
        },
        orb_origin.y + orb_spacing * 3.0,
    );
    render_single_orb(
        app,
        ui,
        &painter,
        spec_pos,
        orb_radius,
        ids::SPEC_ORB,
        ids::SPEC_ICON,
        1.0,
        Color32::from_rgb(30, 150, 40),
        "100",
        "Special",
        "Special Attack: 100%".to_owned(),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_single_orb(
    app: &mut EditorApp,
    ui: &egui::Ui,
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    filler_sprite: u32,
    icon_sprite: u32,
    fill_fraction: f32,
    fallback_color: Color32,
    text: &str,
    label: &str,
    hover_tip: String,
    clickable_id: Option<egui::Id>,
) -> bool {
    let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    let orb_rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    painter.circle_filled(center, radius, Color32::from_rgb(18, 16, 14));

    let frac = fill_fraction.clamp(0.0, 1.0);
    let fill_top = orb_rect.bottom() - (orb_rect.height() * frac);
    let fill_rect = Rect::from_min_max(Pos2::new(orb_rect.left(), fill_top), orb_rect.max);

    if let Some(tex) = app.assets.sprite(filler_sprite).map(|t| t.id()) {
        let uv_rect = Rect::from_min_max(Pos2::new(0.0, 1.0 - frac), Pos2::new(1.0, 1.0));
        painter.image(tex, fill_rect, uv_rect, Color32::WHITE);
    } else {
        painter
            .with_clip_rect(fill_rect)
            .circle_filled(center, radius - 1.0, fallback_color);
    }

    if let Some(tex) = app.assets.sprite(ids::ORB_FRAME).map(|t| t.id()) {
        painter.image(
            tex,
            Rect::from_center_size(center, Vec2::splat(radius * 2.15)),
            full_uv,
            Color32::WHITE,
        );
    } else {
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(2.0, Color32::from_rgb(85, 75, 65)),
        );
    }

    if let Some(tex) = app.assets.sprite(icon_sprite).map(|t| t.id()) {
        painter.image(
            tex,
            Rect::from_center_size(center, Vec2::splat(radius * 1.1)),
            full_uv,
            Color32::WHITE,
        );
    }

    let badge_rect = Rect::from_min_size(
        Pos2::new(center.x - radius - 18.0, center.y - 7.0),
        Vec2::new(20.0, 14.0),
    );
    painter.rect_filled(
        badge_rect,
        3.0,
        Color32::from_rgba_unmultiplied(10, 8, 8, 220),
    );
    painter.rect_stroke(
        badge_rect,
        3.0,
        Stroke::new(1.0, Color32::from_rgb(70, 60, 50)),
        StrokeKind::Inside,
    );
    painter.text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::monospace(10.0),
        Color32::from_rgb(255, 230, 40),
    );

    let interact_id = clickable_id.unwrap_or_else(|| egui::Id::new(("orb_hover", label)));
    let response = ui.interact(orb_rect, interact_id, egui::Sense::click());
    response.clone().on_hover_text(hover_tip);
    response.clicked()
}
