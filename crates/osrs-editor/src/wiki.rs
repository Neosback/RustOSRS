//! "Look up on the wiki" for the selected object, like RuneLite's Wiki plugin: opens
//! `Special:Lookup` on the OSRS Wiki with the object's id, name and location so the wiki resolves it
//! to the right page (or to the search results when there are several).

use crate::keymap::{ActionDef, Binding};
use crate::plugin::{EditorCommand, EditorPlugin, PluginContext};
use eframe::egui;

const WIKI_BASE: &str = "https://oldschool.runescape.wiki/w/Special:Lookup";

/// Percent-encode a query value (unreserved characters pass through).
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The lookup URL for an object (`plane` is the object's own plane).
pub fn lookup_url(id: u32, name: &str, tile: (i32, i32), plane: u8) -> String {
    format!(
        "{WIKI_BASE}?type=object&id={id}&name={}&x={}&y={}&plane={plane}&utm_source=rustosrs",
        encode(name),
        tile.0,
        tile.1
    )
}

#[derive(Default)]
pub struct WikiPlugin;

impl WikiPlugin {
    pub fn new() -> Self {
        Self
    }

    /// URL for the selected object, if any.
    fn selected_url(cx: &PluginContext) -> Option<String> {
        let object = cx.view.selected?;
        let loc = cx.view.infos.get(&object.region)?.locs.get(object.index)?;
        let definition = &loc.definition;
        let name = definition.name.as_deref().unwrap_or("");
        Some(lookup_url(
            definition.identity.id.get(),
            name,
            loc.tile,
            loc.plane,
        ))
    }

    fn open(url: &str, cx: &mut PluginContext) {
        if let Err(error) = crate::platform::open_url(url) {
            cx.send(EditorCommand::Status(format!(
                "could not open the wiki: {error}"
            )));
        }
    }
}

impl EditorPlugin for WikiPlugin {
    fn name(&self) -> &'static str {
        "wiki"
    }

    fn title(&self) -> &'static str {
        "Wiki"
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, cx: &mut PluginContext) {
        let url = Self::selected_url(cx);
        let response = ui
            .add_enabled(url.is_some(), egui::Button::new(crate::icons::WIKI))
            .on_hover_text("Look up the selected object on the OSRS Wiki")
            .on_disabled_hover_text("Select an object to look it up on the OSRS Wiki");
        if response.clicked()
            && let Some(url) = url
        {
            Self::open(&url, cx);
        }
    }

    fn selection_ui(&mut self, ui: &mut egui::Ui, cx: &mut PluginContext) {
        if let Some(url) = Self::selected_url(cx)
            && ui
                .button(format!("{} Wiki", crate::icons::WIKI))
                .on_hover_text("Look up this object on the OSRS Wiki")
                .clicked()
        {
            Self::open(&url, cx);
        }
    }

    fn actions(&self) -> &'static [ActionDef] {
        const ACTIONS: &[ActionDef] = &[ActionDef::new(
            "wiki.lookup",
            "Look up selection on the wiki",
            "Edit",
            Binding::key(egui::Key::L),
        )];
        ACTIONS
    }

    fn on_action(&mut self, id: &'static str, cx: &mut PluginContext) {
        if id == "wiki.lookup"
            && let Some(url) = Self::selected_url(cx)
        {
            Self::open(&url, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_url_encodes_the_name_and_carries_the_location() {
        let url = lookup_url(1938, "Coffin plate", (2620, 3100), 0);
        assert!(
            url.starts_with(
                "https://oldschool.runescape.wiki/w/Special:Lookup?type=object&id=1938"
            )
        );
        assert!(url.contains("name=Coffin%20plate"));
        assert!(url.contains("x=2620&y=3100&plane=0"));
    }
}
