//! The building generator's form and its latest preview: pure planning against a document.
//!
//! The headless generator lives in `osrs-edit`. This owner holds what the panel edits (size,
//! archetype, seed, kit), the kits available, and the last plan; it never touches the map.
//! Applying a preview is the panel's job and goes through the edit gateway.

#[cfg(not(target_arch = "wasm32"))]
mod candidates;
mod design;
mod footprints;
mod plan;
mod preview;
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) mod reproduction;
use footprints::{FootprintInputs, SizeSource};
use preview::PreviewInput;
use std::sync::Arc;
#[cfg(test)]
mod tests;
pub(in crate::app) use design::GenerationMode;
use osrs_core::{
    coords::{MapTile, SourcePlane},
    ids::ObjectId,
};
use osrs_edit::{
    BuildingArchetype, BuildingConstraints, BuildingKit, BuildingRequest, BuildingStyle, KitFile,
    LocKind, MapDocument, MapOp, MapPatch, Prefab, PrefabSource, RectSite,
    capture_generation_snapshot, prefab_from_patch,
};

const STARTER_KITS: [(&str, &str); 2] = [
    (
        "the Lumbridge brick kit",
        include_str!("kits/lumbridge-brick.kit.json"),
    ),
    (
        "the Timber town kit",
        include_str!("kits/timber-town.kit.json"),
    ),
];

/// Largest site side the panel offers, in tiles.
pub(in crate::app) const MAX_SITE: u16 = 48;

pub(in crate::app) const ARCHETYPES: [(BuildingArchetype, &str); 8] = [
    (BuildingArchetype::House, "House"),
    (BuildingArchetype::Shop, "Shop"),
    (BuildingArchetype::Tavern, "Tavern"),
    (BuildingArchetype::Bank, "Bank"),
    (BuildingArchetype::Church, "Church"),
    (BuildingArchetype::Warehouse, "Warehouse"),
    (BuildingArchetype::Castle, "Castle"),
    (BuildingArchetype::Dungeon, "Dungeon"),
];

pub(in crate::app) const STYLES: [(BuildingStyle, &str); 4] = [
    (BuildingStyle::Generic, "Generic"),
    (BuildingStyle::Lumbridge, "Lumbridge"),
    (BuildingStyle::Varrock, "Varrock"),
    (BuildingStyle::Falador, "Falador"),
];

/// What the form asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(target_arch = "wasm32"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(not(target_arch = "wasm32"), serde(deny_unknown_fields))]
pub(in crate::app) struct BuildForm {
    pub(in crate::app) archetype: BuildingArchetype,
    pub(in crate::app) style: BuildingStyle,
    pub(in crate::app) width: u16,
    pub(in crate::app) height: u16,
    pub(in crate::app) floors: u8,
    pub(in crate::app) seed: u64,
    /// Re-rolls windows and doors only; rooms stay put.
    pub(in crate::app) variation: u32,
    pub(in crate::app) kit: usize,
    /// Remove the objects already on the site (and the plane above it) when applying.
    pub(in crate::app) clear_site: bool,
    pub(in crate::app) name: String,
    pub(in crate::app) mode: GenerationMode,
    #[cfg_attr(
        not(target_arch = "wasm32"),
        serde(deserialize_with = "reproduction::form::required_option")
    )]
    pub(in crate::app) stage: Option<osrs_edit::BuildingStage>,
    #[cfg_attr(
        not(target_arch = "wasm32"),
        serde(deserialize_with = "reproduction::form::required_option")
    )]
    pub(in crate::app) sketch: Option<osrs_edit::BuildingSketch>,
    #[cfg_attr(not(target_arch = "wasm32"), serde(with = "reproduction::form::locks"))]
    pub(in crate::app) locks: Vec<osrs_edit::ObjectPlacement>,
    #[cfg_attr(
        not(target_arch = "wasm32"),
        serde(with = "reproduction::form::region")
    )]
    pub(in crate::app) region: Option<RectSite>,
}

impl Default for BuildForm {
    fn default() -> Self {
        Self {
            archetype: BuildingArchetype::House,
            style: BuildingStyle::Generic,
            width: 12,
            height: 10,
            floors: 1,
            seed: 1,
            variation: 0,
            kit: 0,
            clear_site: true,
            name: String::new(),
            mode: GenerationMode::Generate,
            stage: None,
            sketch: None,
            locks: vec![],
            region: None,
        }
    }
}

/// One generated building, ready to apply or keep.
#[derive(Debug, Clone)]
pub(in crate::app) struct Preview {
    input: Arc<PreviewInput>,
    pub(in crate::app) origin: MapTile,
    pub(in crate::app) base_plane: SourcePlane,
    pub(in crate::app) form: BuildForm,
    /// The building alone (what a prefab of it holds).
    pub(in crate::app) building: MapPatch,
    /// Clearing the site, then the building.
    pub(in crate::app) apply: MapPatch,
    pub(in crate::app) objects: usize,
    pub(in crate::app) removed: usize,
    /// Pieces already standing exactly as wanted, left alone.
    pub(in crate::app) standing: usize,
    pub(in crate::app) verdict: osrs_lab::Verdict,
    pub(in crate::app) thumbnail: Prefab,
}

#[derive(Clone)]
pub(in crate::app) struct Kit {
    pub(in crate::app) name: String,
    kit: BuildingKit,
    file: KitFile,
}

pub(in crate::app) struct BuildState {
    pub(in crate::app) form: BuildForm,
    kits: Vec<Kit>,
    kit_problems: Vec<String>,
    /// The site's south-west tile once chosen.
    pub(in crate::app) site: Option<MapTile>,
    /// The plane the site was chosen on: the walls go here, the roof one plane up. Looking at
    /// another plane afterwards does not move the building.
    pub(in crate::app) site_plane: u8,
    /// The next viewport click chooses the site.
    pub(in crate::app) choosing: bool,
    pub(super) preview: Option<Preview>,
    pub(in crate::app) error: Option<String>,
    /// What the preview was made from, so it refreshes only when this changes.
    made_from: Option<Arc<PreviewInput>>,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<candidates::Job>,
    #[cfg(not(target_arch = "wasm32"))]
    candidate_worker: Option<candidates::Worker>,
    #[cfg(not(target_arch = "wasm32"))]
    reproduction_export: Option<reproduction::ExportTask>,
    pub(super) candidates: Vec<Preview>,
    failures: Vec<FailedAttempt>,
}

/// A rejected attempt retains its actual request key and candidate seed, without a map snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FailedAttempt {
    input: Arc<PreviewInput>,
    form: BuildForm,
    failure: plan::Failure,
}

impl Default for BuildState {
    fn default() -> Self {
        let mut state = Self {
            form: BuildForm::default(),
            kits: Vec::new(),
            kit_problems: Vec::new(),
            site: None,
            site_plane: 0,
            choosing: false,
            preview: None,
            error: None,
            made_from: None,
            #[cfg(not(target_arch = "wasm32"))]
            pending: None,
            #[cfg(not(target_arch = "wasm32"))]
            candidate_worker: None,
            #[cfg(not(target_arch = "wasm32"))]
            reproduction_export: None,
            candidates: vec![],
            failures: vec![],
        };
        for (source, text) in STARTER_KITS {
            state.add_kit_text(source, text);
        }
        state
    }
}

impl BuildState {
    #[cfg(test)]
    pub(in crate::app) fn refresh(
        &mut self,
        document: &MapDocument,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) {
        self.refresh_source(document, 0, revision, size_of);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while self.pending.is_some() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(2));
                self.refresh_source(document, 0, revision, size_of);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn learned_roles(&self) -> Option<osrs_edit::LearnedRoleModel> {
        self.kits.get(self.form.kit)?.kit.learned_roles().cloned()
    }

    /// Add a kit from file text; a bad kit is reported, never dropped silently.
    pub(in crate::app) fn add_kit_text(&mut self, source: &str, text: &str) -> bool {
        let compiled = serde_json::from_str::<KitFile>(text)
            .map_err(|error| error.to_string())
            .and_then(|file| {
                file.compile()
                    .map(|kit| (kit, file))
                    .map_err(|error| error.to_string())
            });
        match compiled {
            Ok((kit, file)) => {
                self.kits.push(Kit {
                    name: kit.name().to_owned(),
                    kit,
                    file,
                });
                true
            }
            Err(error) => {
                self.kit_problems.push(format!("{source}: {error}"));
                false
            }
        }
    }

    pub(in crate::app) fn kits(&self) -> &[Kit] {
        &self.kits
    }

    pub(in crate::app) fn kit_problems(&self) -> &[String] {
        &self.kit_problems
    }

    /// Read `*.kit.json` files from `directory` (native).
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn load_kits(&mut self, directory: &std::path::Path) {
        let Ok(read) = std::fs::read_dir(directory) else {
            return;
        };
        let mut files: Vec<_> = read
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.ends_with(".kit.json"))
            .collect();
        files.sort();
        for file in files {
            match std::fs::read_to_string(directory.join(&file)) {
                Ok(text) => {
                    self.add_kit_text(&file, &text);
                }
                Err(error) => self.kit_problems.push(format!("{file}: {error}")),
            }
        }
    }

    /// The rectangle the site covers once chosen.
    pub(in crate::app) fn site_rect(&self) -> Option<(MapTile, u16, u16)> {
        self.site
            .map(|origin| (origin, self.form.width, self.form.height))
    }

    /// Choose the site so its centre is on `tile`.
    pub(in crate::app) fn centre_site_on(&mut self, tile: (i32, i32), plane: u8) {
        self.site_plane = plane;
        self.site = Some(MapTile::new(
            tile.0 - i32::from(self.form.width) / 2,
            tile.1 - i32::from(self.form.height) / 2,
        ));
        self.choosing = false;
    }

    /// The next seed in a fixed sequence (a reroll is repeatable).
    pub(in crate::app) fn reroll(&mut self) {
        self.form.seed = self
            .form
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.form.variation = 0;
    }

    pub(in crate::app) fn to_prefab(
        &self,
        source: u64,
        revision: u64,
        size_of: &SizeSource<'_>,
    ) -> Result<Prefab, String> {
        let preview = self
            .current_preview(source, revision, size_of)
            .ok_or("generate a current building preview first")?;
        let form = &preview.form;
        let archetype = ARCHETYPES
            .iter()
            .find(|(a, _)| *a == form.archetype)
            .map_or("House", |(_, label)| *label);
        let style = STYLES
            .iter()
            .find(|(s, _)| *s == form.style)
            .map_or("Generic", |(_, label)| *label);
        let name = if form.name.trim().is_empty() {
            format!("{archetype} {}", form.seed % 10_000)
        } else {
            form.name.trim().to_owned()
        };
        let mut prefab = prefab_from_patch(
            &preview.building,
            preview.origin,
            preview.base_plane,
            form.width,
            form.height,
            name,
            PrefabSource::Generated {
                archetype: archetype.to_owned(),
                style: style.to_owned(),
                seed: form.seed,
                floors: form.floors.clamp(1, 3),
            },
        )
        .map_err(|error| error.to_string())?;
        prefab.tags = vec!["generated".to_owned(), archetype.to_lowercase()];
        Ok(prefab)
    }
}

/// Say why generation failed in words.
fn describe(error: &osrs_edit::KitGenerateError) -> String {
    use osrs_edit::{BuildingGenerationError as G, KitGenerateError as K};
    match error {
        K::Design(message) => message.to_string(),
        K::MissingStairs => {
            "this kit needs reviewed up/down stair assets before generating multiple floors"
                .to_owned()
        }
        K::Stairs(error) => format!("the stairs could not be placed ({error:?})"),
        K::RoofPlane { plane } => format!("the roof would need plane {plane}, above the top plane"),
        K::Generation(G::UnsupportedFloorCount { floors }) => {
            format!("{floors} floors are not supported yet")
        }
        K::Generation(G::Layout(_)) => "the rooms do not fit this site; make it bigger".to_owned(),
        K::Generation(G::Detail(osrs_edit::BuildingDetailError::MissingSnapshotTerrain {
            tile,
        })) => {
            format!(
                "the map has no height data at {}, {}; move the camera over the site so it loads",
                tile.x, tile.y
            )
        }
        K::Generation(other) => format!("the generator could not plan this site ({other:?})"),
    }
}

fn describe_apply(error: &osrs_edit::PatchApplyError) -> String {
    use osrs_edit::PatchApplyError as E;
    match error {
        E::ObjectAlreadyPresent { placement, .. } => format!(
            "object {} is already at {}, {}",
            placement.object_id.get(),
            placement.position.tile.x,
            placement.position.tile.y
        ),
        E::HeightMissing { coord, .. } => format!(
            "the map has no height at {}, {}; move the camera over the site so it loads",
            coord.corner.x, coord.corner.y
        ),
        other => format!("{other:?}"),
    }
}

pub(in crate::app) fn starter_kit(index: usize) -> Result<KitFile, String> {
    serde_json::from_str(STARTER_KITS.get(index).ok_or("unknown starter kit")?.1)
        .map_err(|e| e.to_string())
}
