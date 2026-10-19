//! Chatbox frame with OSRS messages, stone tab filters, and minimize toggle.

use super::EditorApp;
use crate::camera::CameraMode;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

pub(super) fn draw(app: &mut EditorApp, ui: &egui::Ui, rect: Rect) {
    let width = 450.0;
    let height = if app.ui.chatbox_minimized {
        24.0
    } else {
        130.0
    };
    let box_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 8.0, rect.bottom() - height - 8.0),
        Vec2::new(width, height),
    );
    let painter = ui.painter().with_clip_rect(rect);

    painter.rect_filled(
        box_rect,
        4.0,
        Color32::from_rgba_unmultiplied(18, 16, 14, 210),
    );
    painter.rect_stroke(
        box_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(70, 60, 50)),
        StrokeKind::Inside,
    );

    let toggle_rect = Rect::from_min_size(
        Pos2::new(box_rect.right() - 20.0, box_rect.top() + 3.0),
        Vec2::new(16.0, 16.0),
    );
    let toggle_resp = ui.interact(
        toggle_rect,
        egui::Id::new("chatbox_toggle"),
        egui::Sense::click(),
    );
    painter.text(
        toggle_rect.center(),
        egui::Align2::CENTER_CENTER,
        if app.ui.chatbox_minimized {
            "▲"
        } else {
            "▼"
        },
        egui::FontId::monospace(10.0),
        Color32::from_rgb(200, 200, 200),
    );
    if toggle_resp.clicked() {
        app.ui.chatbox_minimized = !app.ui.chatbox_minimized;
    }

    if app.ui.chatbox_minimized {
        painter.text(
            Pos2::new(box_rect.left() + 10.0, box_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "Chatbox (Minimized)",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(180, 180, 180),
        );
        return;
    }

    let mut y = box_rect.top() + 8.0;
    let line_height = 16.0;
    painter.text(
        Pos2::new(box_rect.left() + 10.0, y),
        egui::Align2::LEFT_TOP,
        "Welcome to RustOSRS.",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(255, 255, 0),
    );
    y += line_height;

    let in_game = app.interaction.camera().mode == CameraMode::Game;
    let mode_str = if in_game {
        "Game Mode: Arrow keys rotate camera, Left-click walks to tile."
    } else {
        "Editor Mode: Switch camera to Game mode to playtest player walking."
    };
    painter.text(
        Pos2::new(box_rect.left() + 10.0, y),
        egui::Align2::LEFT_TOP,
        mode_str,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(120, 200, 255),
    );
    y += line_height;

    let player = &app.settings.prefs.player;
    if let Some(stand) = player.stand {
        let msg = format!(
            "{} standing at ({}, {}, plane {}). Run: {}.",
            player.name,
            stand.x,
            stand.y,
            stand.plane,
            if player.running { "ON" } else { "OFF" }
        );
        painter.text(
            Pos2::new(box_rect.left() + 10.0, y),
            egui::Align2::LEFT_TOP,
            msg,
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    }

    let btn_labels = [
        "All", "Game", "Public", "Private", "Channel", "Clan", "Trade",
    ];
    let btn_w = (width - 16.0) / btn_labels.len() as f32;
    let bar_top = box_rect.bottom() - 22.0;
    for (i, label) in btn_labels.iter().enumerate() {
        let btn_rect = Rect::from_min_size(
            Pos2::new(box_rect.left() + 8.0 + (i as f32) * btn_w, bar_top),
            Vec2::new(btn_w - 2.0, 18.0),
        );
        painter.rect_filled(
            btn_rect,
            2.0,
            Color32::from_rgba_unmultiplied(50, 45, 40, 180),
        );
        painter.rect_stroke(
            btn_rect,
            2.0,
            Stroke::new(1.0, Color32::from_rgb(80, 70, 60)),
            StrokeKind::Inside,
        );
        painter.text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(10.5),
            Color32::from_rgb(220, 220, 220),
        );
    }
}
