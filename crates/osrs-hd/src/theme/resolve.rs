//! Stable property-wise precedence; provenance is retained per property.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct AppearanceQuery {
    pub tile: MapTile,
    pub plane: PlaneIndex,
    pub underlay: Option<FloorUnderlayId>,
    pub overlay: Option<FloorOverlayId>,
    pub object: Option<ObjectId>,
    pub kind: Option<u8>,
    pub orientation: Option<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTheme {
    pub values: ThemeOverrides,
    pub winning_rules: BTreeMap<&'static str, String>,
}
impl ThemeTarget {
    pub fn matches(&self, query: &AppearanceQuery) -> bool {
        match *self {
            Self::Global => true,
            Self::Area {
                min,
                max,
                min_plane,
                max_plane,
            } => {
                (min.x..=max.x).contains(&query.tile.x)
                    && (min.y..=max.y).contains(&query.tile.y)
                    && (min_plane..=max_plane).contains(&query.plane)
            }
            Self::Underlay(id) => query.underlay == Some(id),
            Self::Overlay(id) => query.overlay == Some(id),
            Self::Object(id) => query.object == Some(id),
            Self::Placement {
                tile,
                plane,
                object,
                kind,
                orientation,
            } => {
                query.tile == tile
                    && query.plane == plane
                    && query.object == Some(object)
                    && query.kind == Some(kind)
                    && query.orientation == Some(orientation)
            }
        }
    }
}
impl ThemeDefinition {
    pub fn resolve(&self, project: &ThemeOverrides, query: &AppearanceQuery) -> ResolvedTheme {
        let mut result = ResolvedTheme {
            values: ThemeOverrides::default(),
            winning_rules: BTreeMap::new(),
        };
        result.apply(&self.overrides, "Base theme");
        result.apply(project, "Project");
        for rule in &self.rules {
            if rule.enabled && rule.target.matches(query) {
                result.apply(&rule.values, &rule.name);
            }
        }
        result
    }
}
impl ResolvedTheme {
    fn apply(&mut self, values: &ThemeOverrides, name: &str) {
        macro_rules! provenance { ($($field:ident),*) => { $(if values.$field.is_some() { self.winning_rules.insert(stringify!($field), name.to_owned()); })* }; }
        provenance!(
            ambient_color,
            ambient_strength,
            sunlight_color,
            sunlight_strength,
            fog_color,
            fog_depth,
            water_color,
            exposure,
            brightness,
            normal_mapping,
            parallax_mapping,
            underwater_caustics,
            material
        );
        self.values.overlay(values);
    }
}
impl ThemeOverrides {
    /// Project authored presentation onto disposable lighting. Semantic scene colors stay intact.
    pub fn apply_lighting(&self, lighting: &mut crate::environment::Lighting, draw_distance: f32) {
        use crate::environment::{hsv_to_srgb, linear_to_srgb3, srgb_to_hsv, srgb_to_linear3};
        let linear = |rgb: [u8; 3]| srgb_to_linear3(rgb.map(|c| f32::from(c) / 255.0));
        if let Some(color) = self.ambient_color {
            lighting.ambient_color = linear(color);
        }
        if let Some(strength) = self.ambient_strength {
            lighting.ambient_strength = strength.get();
        }
        if let Some(color) = self.sunlight_color {
            lighting.light_color = linear(color);
        }
        if let Some(strength) = self.sunlight_strength {
            lighting.light_strength = strength.get();
            lighting.casts_shadows = strength.get() > 0.0;
        }
        if let Some(color) = self.fog_color {
            lighting.fog_color = linear(color);
        }
        if let Some(depth) = self.fog_depth {
            lighting.fog_depth = depth.get() * draw_distance.min(90.0) / 10.0;
        }
        if let Some(color) = self.water_color {
            lighting.water_color = linear(color);
            let hsv = srgb_to_hsv(lighting.water_color);
            let shade =
                |factor: f32| linear_to_srgb3(hsv_to_srgb([hsv[0], hsv[1], hsv[2] * factor]));
            lighting.water_color_light = shade(0.8);
            lighting.water_color_mid = shade(0.45);
            lighting.water_color_dark = shade(0.05);
        }
    }
}
