use super::*;
use catalog::Applies::Hd;

#[test]
fn every_row_is_declared_once_with_a_distinct_label_per_section() {
    for (index, def) in ROWS.iter().enumerate() {
        assert!(
            !ROWS[..index].iter().any(|other| other.row == def.row),
            "{} is listed twice",
            def.label
        );
        assert!(
            !ROWS[..index].iter().any(|other| {
                other.page == def.page && other.section == def.section && other.label == def.label
            }),
            "{} appears twice in {}",
            def.label,
            def.section
        );
    }
}

#[test]
fn rows_of_a_section_are_contiguous_so_the_page_draws_one_grid_per_section() {
    let mut seen: Vec<(SettingsPage, &str)> = Vec::new();
    let mut last: Option<(SettingsPage, &str)> = None;
    for def in ROWS {
        let key = (def.page, def.section);
        if last != Some(key) {
            assert!(!seen.contains(&key), "section {} is split", def.section);
            seen.push(key);
            last = Some(key);
        }
    }
}

#[test]
fn every_page_with_settings_has_rows() {
    for (page, _, title) in PAGES {
        let has_rows = ROWS.iter().any(|def| def.page == *page);
        // Controls (shortcuts), Colours, Plugin options and Advanced (reset buttons) are drawn by
        // their own code.
        assert!(
            has_rows || matches!(page, Controls | Colors | Plugins | Advanced),
            "{title} has no settings"
        );
    }
}

#[test]
fn search_matches_labels_sections_and_keywords() {
    let find = |query: &str| -> Vec<&'static str> {
        ROWS.iter()
            .filter(|def| matches(def, query))
            .map(|def| def.label)
            .collect()
    };
    assert!(find("shadow").contains(&"Shadow quality"));
    assert!(find("shadow").contains(&"Terrain shadows"));
    // Keywords: the sky colour is found as "background" and "fog colour".
    assert!(find("background").contains(&"Sky colour"));
    // Every word has to match, in any order.
    assert_eq!(
        find("fog depth"),
        vec!["Fog depth", "Fog depth mode", "Static fog depth"]
    );
    // The context menu setting is discoverable by its old names.
    assert!(find("runelite").contains(&"Context menu"));
    assert!(find("choose option").contains(&"Context menu"));
    assert!(find("zzzz").is_empty());
}

#[test]
fn the_quick_settings_are_real_rows_listed_once() {
    let mut seen: Vec<Row> = Vec::new();
    for (group, rows) in QUICK {
        assert!(!rows.is_empty(), "{group} is empty");
        for row in *rows {
            assert!(
                ROWS.iter().any(|def| def.row == *row),
                "a quick row has no table entry"
            );
            assert!(!seen.contains(row), "a quick row is listed twice");
            seen.push(*row);
        }
    }
    // The window keeps every setting; the quick tab is a subset.
    assert!(seen.len() < ROWS.len());
}

#[test]
fn hd_only_rows_are_not_offered_on_the_reference_pages() {
    for def in ROWS {
        if def.applies == Hd {
            assert!(
                matches!(def.page, HdLighting | HdShadowsWater),
                "{} is an HD option on a general page",
                def.label
            );
        }
    }
}
