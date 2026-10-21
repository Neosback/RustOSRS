//! Source model-table evidence, not guessed semantic assemblies or selected render geometry.
use osrs_core::definitions::ObjectModels;
use serde_json::{Value, json};

pub(super) fn metadata(models: Option<&ObjectModels>) -> Value {
    match models {
        None => Value::Null,
        Some(ObjectModels::Untyped(entries)) => json!({
            "representation": "combined",
            "entries": entries.iter().map(|id| id.get()).collect::<Vec<_>>(),
        }),
        Some(ObjectModels::Typed(entries)) => json!({
            "representation": "typed_alternatives",
            "entries": entries.iter().map(|entry| json!({
                "kind": entry.loc_type.get(), "model": entry.model_id.get(),
            })).collect::<Vec<_>>(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osrs_core::{
        definitions::{LocType, TypedObjectModel},
        ids::ModelId,
    };

    #[test]
    fn combined_parts_and_typed_alternatives_keep_order_duplicates_and_full_width_ids() {
        let combined = ObjectModels::Untyped(vec![
            ModelId::new(u32::MAX),
            ModelId::new(7),
            ModelId::new(7),
        ]);
        assert_eq!(
            metadata(Some(&combined)),
            json!({
                "representation": "combined", "entries": [u32::MAX, 7, 7],
            })
        );
        let typed = ObjectModels::Typed(vec![
            TypedObjectModel {
                loc_type: LocType::new(0),
                model_id: ModelId::new(8),
            },
            TypedObjectModel {
                loc_type: LocType::new(10),
                model_id: ModelId::new(9),
            },
        ]);
        assert_eq!(
            metadata(Some(&typed)),
            json!({
                "representation": "typed_alternatives",
                "entries": [{"kind": 0, "model": 8}, {"kind": 10, "model": 9}],
            })
        );
        assert_eq!(metadata(None), Value::Null);
    }
}
