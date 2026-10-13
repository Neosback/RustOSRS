//! The channel tab strip and its icons.

use super::*;

/// The icon of a painter channel.
pub(super) fn channel_icon(tab: DrawerTab) -> &'static str {
    match tab {
        DrawerTab::Overlay => icons::CH_OVERLAY,
        DrawerTab::Underlay => icons::CH_UNDERLAY,
        DrawerTab::Shape => icons::CH_SHAPE,
        DrawerTab::Height => icons::CH_HEIGHT,
        DrawerTab::Mask => icons::CH_MASK,
    }
}

/// A painter channel as a folder tab: the tick on its left arms the channel (a filled accent disc when
/// the brush sets it, an empty ring when each tile keeps its own), the rest of the tab shows its
/// palette. The selected tab is raised, accent-capped and joins the panel below. Returns whether the
/// tab (not the tick) was clicked, and the tab's rectangle.
pub(super) fn channel_tab(
    ui: &mut egui::Ui,
    selected: bool,
    tab: DrawerTab,
    app: &mut EditorApp,
) -> (bool, Rect) {
    let visuals = ui.visuals().clone();
    let accent = visuals.selection.stroke.color;
    let armed = app.prefs.brush.is_armed(tab);
    let galley = ui.painter().layout_no_wrap(
        tab.label().to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        Color32::PLACEHOLDER,
    );
    let size = Vec2::new(34.0 + 20.0 + galley.size().x + 12.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let tick_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 18.0, rect.center().y + 1.0),
        Vec2::splat(22.0),
    );
    let tick = ui.interact(tick_rect, response.id.with("tick"), Sense::click());
    let on_tick = tick.hovered();
    let on_tab = response.hovered() && !on_tick;
    let fill = if selected {
        visuals.widgets.inactive.bg_fill
    } else if on_tab {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        // Between the inset and the widget fill: tabs read as tabs without going black.
        visuals
            .extreme_bg_color
            .lerp_to_gamma(visuals.widgets.inactive.bg_fill, 0.45)
    };
    let painter = ui.painter();
    let shape = CornerRadius {
        nw: 8,
        ne: 8,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(rect, shape, fill);
    painter.rect_stroke(
        rect,
        shape,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        StrokeKind::Inside,
    );
    if selected {
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left() + 6.0, rect.top()),
                egui::pos2(rect.right() - 6.0, rect.top() + 2.5),
            ),
            1.25,
            accent,
        );
    }
    // The tick.
    let centre = tick_rect.center();
    if armed {
        painter.circle_filled(centre, 9.0, accent);
        // The check is drawn as a bold stroke in black or white, whichever the accent calls for.
        painter.add(egui::Shape::line(
            vec![
                centre + Vec2::new(-4.0, 0.5),
                centre + Vec2::new(-1.2, 3.4),
                centre + Vec2::new(4.2, -3.0),
            ],
            Stroke::new(2.3, crate::theme::contrast_on(accent)),
        ));
    } else {
        painter.circle_stroke(
            centre,
            8.0,
            Stroke::new(
                1.6,
                if on_tick {
                    accent
                } else {
                    visuals.weak_text_color().gamma_multiply(0.8)
                },
            ),
        );
    }
    // Armed channels read at full strength (a tab you are not on only a little less); disarmed ones
    // dim, since nothing will be painted there.
    let tint = match (armed, selected) {
        (true, true) => visuals.text_color(),
        (true, false) => visuals.text_color().gamma_multiply(0.85),
        (false, _) => visuals.weak_text_color(),
    };
    painter.text(
        egui::pos2(rect.left() + 38.0, rect.center().y + 1.0),
        egui::Align2::LEFT_CENTER,
        channel_icon(tab),
        egui::FontId::proportional(15.0),
        if selected && armed { accent } else { tint },
    );
    painter.galley(
        egui::pos2(
            rect.left() + 58.0,
            rect.center().y + 1.0 - galley.size().y / 2.0,
        ),
        galley,
        tint,
    );
    let tick = tick.on_hover_text(if armed {
        "The brush sets this. Click to keep each tile's own instead."
    } else {
        "Each tile keeps its own. Click to paint this channel."
    });
    let mut tab_clicked = false;
    if tick.clicked() {
        app.prefs.brush.set_armed(tab, !armed);
    } else if response.clicked() && !on_tick {
        tab_clicked = true;
    }
    (tab_clicked, rect)
}
