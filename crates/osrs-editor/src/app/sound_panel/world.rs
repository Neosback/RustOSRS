//! The sounds in the viewport: a marker on each object that plays one, the full-volume and
//! audible ranges of the selected one, and opening an object's page by clicking its marker.
use crate::{
    app::EditorApp,
    settings::RailTab,
    sound::{self, AreaSound},
};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// A click this close (in points) to a marker picks it.
const PICK_RADIUS: f32 = 12.0;

/// Markers show while the Sound panel is open (unless hidden there) or when the setting asks.
fn visible(app: &EditorApp) -> bool {
    let prefs = &app.settings.prefs;
    prefs.sound_overlay
        || (prefs.properties_panel
            && prefs.rail_tab == RailTab::Sound
            && !app.ui.sound.hide_in_world)
}

/// A world point (tile units) projected to the viewport, lifted `lift` units above the ground.
fn project(
    app: &EditorApp,
    viewport: Rect,
    pixels_per_point: f32,
    size: (u32, u32),
    point: (f32, f32),
    plane: u8,
    lift: f64,
) -> Option<Pos2> {
    let tile = (point.0.floor() as i32, point.1.floor() as i32);
    let corners = crate::interact::tile_corners(app.world.infos(), tile, plane)?;
    let ground = corners.iter().map(|c| c[1]).sum::<f64>() / 4.0;
    app.screen_point(
        [
            f64::from(point.0) * 128.0,
            ground - lift,
            f64::from(point.1) * 128.0,
        ],
        viewport,
        pixels_per_point,
        size.0,
        size.1,
    )
}

fn centre(row: &AreaSound) -> (f32, f32) {
    let (min, max) = row.rect();
    ((min.0 + max.0) / 2.0, (min.1 + max.1) / 2.0)
}

/// Rows on the viewed plane that pass the panel's filter.
fn rows(app: &EditorApp) -> impl Iterator<Item = &AreaSound> {
    let plane = app.interaction.view_plane();
    let needle = app.ui.sound.filter.trim().to_lowercase();
    app.ui
        .sound
        .cache
        .iter()
        .flat_map(|cache| cache.rows.iter())
        .filter(move |row| row.plane == plane && super::row_matches(app, row, &needle))
}

pub(in crate::app) fn draw_world(
    app: &EditorApp,
    painter: &egui::Painter,
    viewport: Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) {
    if !visible(app) {
        return;
    }
    let accent = Color32::from_rgb(120, 200, 255);
    let size = (width, height);
    for row in rows(app) {
        let selected = app.ui.sound.selected == Some((row.region, row.index));
        let c = centre(row);
        if let Some(at) = project(app, viewport, pixels_per_point, size, c, row.plane, 24.0)
            && viewport.contains(at)
        {
            let playing = row
                .main_sound()
                .is_some_and(|id| app.audio.playing() == Some(id));
            let colour = if selected || playing {
                Color32::WHITE
            } else {
                accent
            };
            painter.circle_filled(
                at,
                if selected { 6.0 } else { 4.0 },
                colour.gamma_multiply(0.9),
            );
            painter.circle_stroke(
                at,
                7.0,
                Stroke::new(1.5, Color32::BLACK.gamma_multiply(0.6)),
            );
        }
        if !selected {
            continue;
        }
        let (min, max) = row.rect();
        let (inner, outer) = sound::reach(&row.sound);
        for (distance, colour) in [(outer, accent), (inner, Color32::from_rgb(120, 255, 170))] {
            let points: Vec<Pos2> = sound::ring(min, max, distance)
                .into_iter()
                .filter_map(|p| project(app, viewport, pixels_per_point, size, p, row.plane, 6.0))
                .collect();
            if points.len() > 2 {
                painter.add(egui::Shape::closed_line(points, Stroke::new(2.0, colour)));
            }
        }
    }
}

/// A click (or Shift+right-click) on a marker opens that object's page in the Sound panel and
/// is consumed, so the active tool does not also act on the object under it.
pub(in crate::app) fn marker_click(
    app: &mut EditorApp,
    response: &egui::Response,
    viewport: Rect,
    pixels_per_point: f32,
    width: u32,
    height: u32,
) -> bool {
    let shift = response.ctx.input(|i| i.modifiers.shift);
    if !visible(app) || !(response.clicked() || (response.secondary_clicked() && shift)) {
        return false;
    }
    let Some(pointer) = response.interact_pointer_pos() else {
        return false;
    };
    let size = (width, height);
    let hit = rows(app)
        .filter_map(|row| {
            let at = project(
                app,
                viewport,
                pixels_per_point,
                size,
                centre(row),
                row.plane,
                24.0,
            )?;
            let distance = at.distance(pointer);
            (distance <= PICK_RADIUS).then_some(((row.region, row.index), distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(key, _)| key);
    let Some(key) = hit else {
        return false;
    };
    app.ui.sound.open_object(key);
    app.settings.prefs.rail_tab = RailTab::Sound;
    app.settings.prefs.properties_panel = true;
    true
}
