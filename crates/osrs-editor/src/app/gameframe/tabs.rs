//! Side-panel stone tab rows with authentic OSRS icons and navigation.

use super::EditorApp;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

mod ids {
    pub const TABS_TOP: u32 = 1173;
    pub const TABS_BOTTOM: u32 = 1174;

    pub const TAB_COMBAT: u32 = 774;
    pub const TAB_STATS: u32 = 775;
    pub const TAB_QUESTS: u32 = 776;
    pub const TAB_INVENTORY: u32 = 777;
    pub const TAB_EQUIPMENT: u32 = 778;
    pub const TAB_PRAYER: u32 = 779;
    pub const TAB_MAGIC: u32 = 780;
    pub const TAB_FRIENDS_CHAT: u32 = 781;
    pub const TAB_ACCOUNT: u32 = 782;
    pub const TAB_FRIENDS: u32 = 783;
    pub const TAB_LOGOUT: u32 = 784;
    pub const TAB_OPTIONS: u32 = 785;
    pub const TAB_EMOTES: u32 = 786;
    pub const TAB_MUSIC: u32 = 787;
}

const TAB_DATA: [(u32, &str); 14] = [
    (ids::TAB_COMBAT, "Combat Options"),
    (ids::TAB_STATS, "Skills"),
    (ids::TAB_QUESTS, "Quest List"),
    (ids::TAB_INVENTORY, "Inventory"),
    (ids::TAB_EQUIPMENT, "Worn Equipment"),
    (ids::TAB_PRAYER, "Prayer"),
    (ids::TAB_MAGIC, "Magic"),
    (ids::TAB_FRIENDS_CHAT, "Chat-channel"),
    (ids::TAB_ACCOUNT, "Account Management"),
    (ids::TAB_FRIENDS, "Friends List"),
    (ids::TAB_LOGOUT, "Logout"),
    (ids::TAB_OPTIONS, "Settings / Character"),
    (ids::TAB_EMOTES, "Emotes"),
    (ids::TAB_MUSIC, "Music Player"),
];

pub(super) fn draw(app: &mut EditorApp, ui: &egui::Ui, rect: Rect) {
    let (tab_w, tab_h) = (32.0, 32.0);
    let (bar_w, bar_h) = (tab_w * 7.0, tab_h * 2.0);
    let bar_rect = Rect::from_min_size(
        Pos2::new(rect.right() - bar_w - 8.0, rect.bottom() - bar_h - 8.0),
        Vec2::new(bar_w, bar_h),
    );
    let painter = ui.painter().with_clip_rect(rect);
    let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

    if let Some(tex) = app.assets.sprite(ids::TABS_TOP).map(|t| t.id()) {
        painter.image(
            tex,
            Rect::from_min_size(bar_rect.min, Vec2::new(bar_w, tab_h)),
            full_uv,
            Color32::WHITE,
        );
    }
    if let Some(tex) = app.assets.sprite(ids::TABS_BOTTOM).map(|t| t.id()) {
        painter.image(
            tex,
            Rect::from_min_size(
                Pos2::new(bar_rect.min.x, bar_rect.min.y + tab_h),
                Vec2::new(bar_w, tab_h),
            ),
            full_uv,
            Color32::WHITE,
        );
    }

    let mut open_studio = false;
    for (idx, &(sprite_id, tooltip)) in TAB_DATA.iter().enumerate() {
        let (col, row) = ((idx % 7) as f32, (idx / 7) as f32);
        let cell = Rect::from_min_size(
            Pos2::new(bar_rect.min.x + col * tab_w, bar_rect.min.y + row * tab_h),
            Vec2::new(tab_w, tab_h),
        );

        if app.ui.selected_gameframe_tab == idx {
            painter.rect_filled(
                cell.shrink(1.0),
                2.0,
                Color32::from_rgba_unmultiplied(160, 60, 30, 90),
            );
            painter.rect_stroke(
                cell.shrink(1.0),
                2.0,
                Stroke::new(1.5, Color32::from_rgb(255, 200, 50)),
                StrokeKind::Inside,
            );
        }

        if let Some(tex) = app.assets.sprite(sprite_id).map(|t| t.id()) {
            painter.image(
                tex,
                Rect::from_center_size(cell.center(), Vec2::splat(22.0)),
                full_uv,
                Color32::WHITE,
            );
        } else {
            painter.text(
                cell.center(),
                egui::Align2::CENTER_CENTER,
                &tooltip[..2.min(tooltip.len())],
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }

        let resp = ui.interact(
            cell,
            egui::Id::new(("stone_tab", idx)),
            egui::Sense::click(),
        );
        if resp.clicked() {
            app.ui.selected_gameframe_tab = idx;
            if idx == 1 || idx == 4 || idx == 11 {
                open_studio = true;
            }
        }
        resp.on_hover_text(tooltip);
    }
    if open_studio {
        app.ui.player_window_open = true;
    }
}
