//! Offline execution of normalized fixtures against production semantic code.

use crate::fixture::FixtureExecution;
use crate::inventory::FixtureInventory;
use crate::loader::LoadedFixture;
use crate::schema::{
    NormalizedExpectedCase, NormalizedFixtureExpected, NormalizedFixtureInput, NormalizedInputCase,
    NormalizedModelPoint, NormalizedModelSelection, NormalizedObjectModels, NormalizedTriangle,
};
use osrs_core::coords::ModelPoint;
use osrs_core::definitions::{
    DefinitionIdentity, LocType, ModelScale, ModelTranslation, ObjectDefinition, ObjectModels,
    ObjectPlacementFlags, RecolorPair, RetexturePair, TypedObjectModel,
};
use osrs_core::ids::{ModelId, ObjectId, TextureId};
use osrs_core::model::{
    FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
};
use osrs_core::model_construction::{
    AssembledModel, apply_object_model_instance_transforms, mirror_source_model,
    select_object_model,
};
use osrs_core::provenance::{CacheFingerprint, ProfileDigest, TargetProvenance};
use std::fmt;

const PROFILE_DIGEST: &str = "cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7";
const CACHE_FINGERPRINT: &str = "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureRunReport {
    fixture_ids: Vec<String>,
}

impl FixtureRunReport {
    pub fn fixture_ids(&self) -> &[String] {
        &self.fixture_ids
    }

    pub fn len(&self) -> usize {
        self.fixture_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixture_ids.is_empty()
    }
}

/// Validate every discovered normalized fixture and execute production-backed
/// fixtures through their owning semantic code. Evidence-only fixtures remain
/// schema/hash/provenance gated but are not falsely treated as implemented.
pub fn run_inventory(inventory: &FixtureInventory) -> Result<FixtureRunReport, FixtureRunError> {
    let mut fixture_ids = Vec::with_capacity(inventory.len());
    for fixture in inventory.fixtures() {
        run_fixture(fixture)?;
        fixture_ids.push(fixture.manifest.fixture_id.clone());
    }
    Ok(FixtureRunReport { fixture_ids })
}

/// Validate one integrity-verified fixture and, when its production owner
/// exists, require exact typed output equality.
pub fn run_fixture(fixture: &LoadedFixture) -> Result<(), FixtureRunError> {
    let fixture_id = fixture.manifest.fixture_id.clone();
    let input = NormalizedFixtureInput::parse(&fixture.input)
        .map_err(|error| failure(&fixture_id, format!("input schema: {error}")))?;
    let expected = NormalizedFixtureExpected::parse(&fixture.expected)
        .map_err(|error| failure(&fixture_id, format!("expected schema: {error}")))?;

    if input.kind() != expected.kind() {
        return Err(failure(
            &fixture_id,
            format!(
                "input/expected normalized kinds differ: input={:?}, expected={:?}",
                input.kind(),
                expected.kind()
            ),
        ));
    }

    if fixture.manifest.execution == FixtureExecution::EvidenceOnly {
        return Ok(());
    }

    let actual = execute_input(&fixture_id, input)?;
    if actual != expected.case {
        return Err(failure(
            &fixture_id,
            format!(
                "exact normalized semantic mismatch\nexpected: {:#?}\nactual: {:#?}",
                expected.case, actual
            ),
        ));
    }
    Ok(())
}

fn execute_input(
    fixture_id: &str,
    input: NormalizedFixtureInput,
) -> Result<NormalizedExpectedCase, FixtureRunError> {
    match input.case {
        NormalizedInputCase::ModelSelection {
            models,
            is_rotated,
            requested_type,
            orientation,
        } => {
            let definition = object_definition(
                Some(convert_models(models)),
                is_rotated,
                Vec::new(),
                Vec::new(),
                ModelScale::IDENTITY,
                ModelTranslation::ZERO,
            )
            .map_err(|detail| failure(fixture_id, detail))?;
            let selection =
                select_object_model(&definition, LocType::new(requested_type), orientation).map(
                    |selection| NormalizedModelSelection {
                        model_ids: selection
                            .model_ids()
                            .iter()
                            .map(|model_id| model_id.get())
                            .collect(),
                        mirror: selection.mirror(),
                    },
                );
            Ok(NormalizedExpectedCase::ModelSelection { selection })
        }
        NormalizedInputCase::ModelMirror { vertices, faces } => {
            let source = source_model(vertices, faces.clone(), vec![0; faces.len()], None)
                .map_err(|detail| failure(fixture_id, detail))?;
            let mirrored = mirror_source_model(&source)
                .map_err(|error| failure(fixture_id, format!("mirror source model: {error}")))?;
            Ok(NormalizedExpectedCase::ModelMirror {
                vertices: mirrored
                    .vertices()
                    .iter()
                    .copied()
                    .map(normalize_point)
                    .collect(),
                faces: mirrored
                    .faces()
                    .iter()
                    .copied()
                    .map(normalize_triangle)
                    .collect(),
            })
        }
        NormalizedInputCase::ModelTransform {
            vertices,
            face_colors,
            face_textures,
            requested_type,
            orientation,
            recolors,
            retextures,
            scale,
            translation,
        } => {
            if face_colors.len() != face_textures.len() {
                return Err(failure(
                    fixture_id,
                    "transform fixture face_colors and face_textures lengths differ",
                ));
            }
            if !face_colors.is_empty() && vertices.is_empty() {
                return Err(failure(
                    fixture_id,
                    "transform fixture has face metadata but no vertices",
                ));
            }

            let faces = (0..face_colors.len())
                .map(|_| NormalizedTriangle { a: 0, b: 0, c: 0 })
                .collect();
            let source = source_model(vertices, faces, face_colors, Some(face_textures))
                .map_err(|detail| failure(fixture_id, detail))?;
            let mut assembled = AssembledModel::from_source(&source);
            let definition = object_definition(
                None,
                false,
                recolors
                    .into_iter()
                    .map(|pair| RecolorPair {
                        from: pair.from,
                        to: pair.to,
                    })
                    .collect(),
                retextures
                    .into_iter()
                    .map(|pair| RetexturePair {
                        from: pair.from,
                        to: pair.to,
                    })
                    .collect(),
                ModelScale {
                    x: scale.x,
                    y: scale.y,
                    z: scale.z,
                },
                ModelTranslation {
                    x: translation.x,
                    y: translation.y,
                    z: translation.z,
                },
            )
            .map_err(|detail| failure(fixture_id, detail))?;
            apply_object_model_instance_transforms(
                &mut assembled,
                &definition,
                LocType::new(requested_type),
                orientation,
            );

            Ok(NormalizedExpectedCase::ModelTransform {
                vertices: assembled
                    .vertices()
                    .iter()
                    .copied()
                    .map(normalize_point)
                    .collect(),
                face_colors: assembled.face_colors().to_vec(),
                face_textures: assembled
                    .face_textures()
                    .unwrap_or_default()
                    .iter()
                    .map(|texture| texture.map(|texture| texture.get()))
                    .collect(),
            })
        }
        NormalizedInputCase::PriorityOrder { .. } => Err(failure(
            fixture_id,
            "priority_order has no production executor yet; manifest must use evidence_only",
        )),
    }
}

fn convert_models(models: NormalizedObjectModels) -> ObjectModels {
    match models {
        NormalizedObjectModels::Typed { entries } => ObjectModels::Typed(
            entries
                .into_iter()
                .map(|entry| TypedObjectModel {
                    loc_type: LocType::new(entry.loc_type),
                    model_id: ModelId::new(entry.model_id),
                })
                .collect(),
        ),
        NormalizedObjectModels::Untyped { model_ids } => {
            ObjectModels::Untyped(model_ids.into_iter().map(ModelId::new).collect())
        }
    }
}

fn object_definition(
    models: Option<ObjectModels>,
    is_rotated: bool,
    recolors: Vec<RecolorPair>,
    retextures: Vec<RetexturePair>,
    scale: ModelScale,
    translation: ModelTranslation,
) -> Result<ObjectDefinition, String> {
    Ok(ObjectDefinition {
        identity: DefinitionIdentity::new(ObjectId::new(0), fixture_provenance()?),
        name: None,
        models,
        size_x: 1,
        size_y: 1,
        placement: ObjectPlacementFlags {
            interact_type: 0,
            blocks_projectiles: false,
            clipped: false,
            model_clipped: false,
            obstructs_ground: false,
            solid: false,
        },
        decoration_displacement: 16,
        support_items: None,
        is_rotated,
        non_flat_shading: false,
        contour_clip: None,
        animation: None,
        ambient: 0,
        contrast: 0,
        scale,
        translation,
        recolors,
        retextures,
        morphs: None,
        map_scene: None,
        map_icon: None,
        category: None,
        actions: [None, None, None, None, None],
    })
}

fn source_model(
    vertices: Vec<NormalizedModelPoint>,
    faces: Vec<NormalizedTriangle>,
    face_colors: Vec<u16>,
    face_textures: Option<Vec<Option<u32>>>,
) -> Result<SourceModel, String> {
    let vertices = vertices
        .into_iter()
        .map(|point| ModelPoint::new(point.x, point.y, point.z))
        .collect();
    let faces = faces
        .into_iter()
        .map(|triangle| Triangle::new(triangle.a, triangle.b, triangle.c))
        .collect();
    let face_textures = face_textures.map(|textures| {
        textures
            .into_iter()
            .map(|texture| texture.map(TextureId::new))
            .collect()
    });

    SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(0), fixture_provenance()?),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: None,
        },
        vertices,
        faces,
        face_colors,
        default_priority: FacePriority::ZERO,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: Vec::new(),
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })
    .map_err(|error| format!("construct fixture source model: {error}"))
}

fn fixture_provenance() -> Result<TargetProvenance, String> {
    let profile_digest = ProfileDigest::from_lower_hex(PROFILE_DIGEST)
        .map_err(|error| format!("parse fixture profile digest: {error}"))?;
    let cache_fingerprint = CacheFingerprint::from_lower_hex(CACHE_FINGERPRINT)
        .map_err(|error| format!("parse fixture cache fingerprint: {error}"))?;
    TargetProvenance::new("osrs-live-build-241", profile_digest, cache_fingerprint, 1)
        .map_err(|error| format!("construct fixture target provenance: {error}"))
}

const fn normalize_point(point: ModelPoint) -> NormalizedModelPoint {
    NormalizedModelPoint {
        x: point.x,
        y: point.y,
        z: point.z,
    }
}

const fn normalize_triangle(triangle: Triangle) -> NormalizedTriangle {
    NormalizedTriangle {
        a: triangle.a.get(),
        b: triangle.b.get(),
        c: triangle.c.get(),
    }
}

fn failure(fixture_id: &str, detail: impl Into<String>) -> FixtureRunError {
    FixtureRunError {
        fixture_id: fixture_id.to_owned(),
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureRunError {
    fixture_id: String,
    detail: String,
}

impl FixtureRunError {
    pub fn fixture_id(&self) -> &str {
        &self.fixture_id
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for FixtureRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fixture `{}` failed: {}",
            self.fixture_id, self.detail
        )
    }
}

impl std::error::Error for FixtureRunError {}
