use super::*;
use osrs_core::{
    coords::ModelPoint,
    definitions::DefinitionIdentity,
    ids::ModelId,
    model::{ModelEncoding, ModelFormatIdentity, SourceModelParts, Triangle},
    player::{Gender, KIT_PARTS, WearModels, WornItem},
    provenance::{CacheFingerprint, ProfileDigest, TargetProvenance},
};

fn provenance() -> TargetProvenance {
    TargetProvenance::new(
        "test",
        ProfileDigest::from_lower_hex(&"b".repeat(64)).unwrap_or_else(|_| unreachable!()),
        CacheFingerprint::from_lower_hex(&"a".repeat(64)).unwrap_or_else(|_| unreachable!()),
        1,
    )
    .unwrap_or_else(|_| unreachable!())
}

/// A one-triangle model whose face colour and position identify it.
fn model(id: u32, color: u16, y: i32) -> Arc<SourceModel> {
    Arc::new(
        SourceModel::from_parts(SourceModelParts {
            identity: DefinitionIdentity::new(ModelId::new(id), provenance()),
            format: ModelFormatIdentity {
                encoding: ModelEncoding::Legacy,
                version: None,
            },
            vertices: vec![
                ModelPoint::new(0, y, 0),
                ModelPoint::new(64, y, 0),
                ModelPoint::new(0, y, 64),
            ],
            faces: vec![Triangle::new(0, 1, 2)],
            face_colors: vec![color],
            default_priority: osrs_core::model::FacePriority::new(0)
                .unwrap_or_else(|| unreachable!()),
            face_render_types: None,
            face_priorities: None,
            face_alphas: None,
            face_textures: None,
            texture_face_selectors: None,
            face_biases: None,
            texture_triangles: Vec::new(),
            vertex_skins: None,
            face_skins: None,
            skeletal_vertices: None,
        })
        .unwrap_or_else(|_| unreachable!("valid fixture")),
    )
}

struct Fixture(BTreeMap<ModelId, Arc<SourceModel>>);
impl ModelLoader for Fixture {
    fn load(&mut self, id: ModelId) -> Result<Arc<SourceModel>, String> {
        self.0
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("no model {id}"))
    }
}

fn kit(id: u32, model_id: u32, recolors: &[(u16, u16)]) -> (KitId, Arc<KitDefinition>) {
    (
        KitId::new(id),
        Arc::new(KitDefinition {
            identity: DefinitionIdentity::new(KitId::new(id), provenance()),
            body_part: Some(0),
            models: vec![ModelId::new(model_id)],
            non_selectable: false,
            recolors: recolors
                .iter()
                .map(|(from, to)| RecolorPair {
                    from: *from,
                    to: *to,
                })
                .collect(),
            retextures: Vec::new(),
        }),
    )
}

fn item(id: u32, male: u32, offset: u8) -> (ItemId, Arc<ItemDefinition>) {
    (
        ItemId::new(id),
        Arc::new(ItemDefinition {
            identity: DefinitionIdentity::new(ItemId::new(id), provenance()),
            name: None,
            equip_slot: Some(0),
            hides: [None; 2],
            male: WearModels {
                models: [Some(ModelId::new(male)), None, None],
                offset,
            },
            female: WearModels::default(),
            recolors: Vec::new(),
            retextures: Vec::new(),
        }),
    )
}

#[test]
fn adjusting_copies_recolours_and_moves_without_touching_the_original() {
    let original = model(1, 100, 10);
    let changed = adjusted(&original, &[RecolorPair { from: 100, to: 200 }], &[], 5)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(changed.face_colors(), [200]);
    assert_eq!(changed.vertices()[0].y, 15);
    assert_eq!(original.face_colors(), [100]);
    assert_eq!(original.vertices()[0].y, 10);
    // A colour that is not there changes nothing.
    let same = adjusted(&original, &[RecolorPair { from: 7, to: 8 }], &[], 0)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(same.face_colors(), [100]);
}

#[test]
fn slots_join_in_order_and_each_part_recolours_only_its_own_models() {
    let mut loader = Fixture(
        [(10, 500, 0), (20, 500, 0), (30, 600, 0)]
            .iter()
            .map(|(id, color, y)| (ModelId::new(*id), model(*id, *color, *y)))
            .collect(),
    );
    // Hair kit recolours 500 -> 111; the torso kit has the same source colour and no recolour.
    let kits: BTreeMap<_, _> = [kit(1, 10, &[(500, 111)]), kit(2, 20, &[])]
        .into_iter()
        .collect();
    let mut kit_parts = [None; KIT_PARTS];
    kit_parts[0] = Some(KitId::new(1)); // hair, slot 8
    kit_parts[2] = Some(KitId::new(2)); // torso, slot 4
    let appearance = PlayerAppearance::compose(Gender::Male, kit_parts, [200; 5], &[]);
    let built = build_player_base(&mut loader, &appearance, &kits, &BTreeMap::new())
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("two models"));
    assert_eq!(built.face_colors.len(), 2);
    // Slot order puts the torso (4) before the hair (8). Colours are lit values, so compare
    // that the two faces differ: only the hair's source colour was recoloured.
    let (torso, hair) = (built.face_colors[0], built.face_colors[1]);
    assert_ne!(
        torso.a, hair.a,
        "the hair was recoloured, the torso was not"
    );
}

#[test]
fn a_worn_item_with_an_offset_moves_down_and_nothing_means_no_model() {
    let mut loader = Fixture(
        [(ModelId::new(40), model(40, 300, 0))]
            .into_iter()
            .collect(),
    );
    let items: BTreeMap<_, _> = [item(7, 40, 12)].into_iter().collect();
    let worn = WornItem {
        slot: 0,
        item: ItemId::new(7),
        hides: [None; 2],
    };
    let appearance = PlayerAppearance::compose(Gender::Male, [None; KIT_PARTS], [200; 5], &[worn]);
    let built = build_player_base(&mut loader, &appearance, &BTreeMap::new(), &items)
        .unwrap_or_else(|e| unreachable!("{e}"))
        .unwrap_or_else(|| unreachable!("one model"));
    assert!(built.vertices.iter().all(|v| v.y == 12));
    // Female has no model for this item, so the slot shows nothing.
    let female = PlayerAppearance::compose(Gender::Female, [None; KIT_PARTS], [200; 5], &[worn]);
    assert!(
        build_player_base(&mut loader, &female, &BTreeMap::new(), &items)
            .unwrap_or_else(|e| unreachable!("{e}"))
            .is_none()
    );
}

#[test]
fn missing_definitions_and_models_are_errors_not_substitutions() {
    let mut loader = Fixture(BTreeMap::new());
    let mut kit_parts = [None; KIT_PARTS];
    kit_parts[0] = Some(KitId::new(99));
    let appearance = PlayerAppearance::compose(Gender::Male, kit_parts, [0; 5], &[]);
    assert!(
        build_player_base(&mut loader, &appearance, &BTreeMap::new(), &BTreeMap::new()).is_err()
    );
    let kits: BTreeMap<_, _> = [kit(99, 5, &[])].into_iter().collect();
    assert!(build_player_base(&mut loader, &appearance, &kits, &BTreeMap::new()).is_err());
}
