//! Built-in plugin lifecycle and overlay contributions. Plugins receive only a read-only view
//! and return commands; application orchestration applies those commands after callbacks finish.
use crate::plugin::{EditorCommand, EditorPlugin, EditorView, PluginContext};
use osrs_render::gpu::LineVertex;
use std::collections::BTreeMap;
type Commands = Vec<(&'static str, EditorCommand)>;
pub(super) struct PluginRuntime {
    plugins: Vec<Box<dyn EditorPlugin>>,
    lines: BTreeMap<&'static str, Vec<LineVertex>>,
    lines_changed: bool,
}
impl PluginRuntime {
    pub(super) fn new(plugins: Vec<Box<dyn EditorPlugin>>) -> Self {
        Self {
            plugins,
            lines: BTreeMap::new(),
            lines_changed: false,
        }
    }
    pub(super) fn plugins(&self) -> &[Box<dyn EditorPlugin>] {
        &self.plugins
    }
    pub(super) fn run(
        &mut self,
        view: EditorView<'_>,
        disabled: &[String],
        mut callback: impl FnMut(&mut dyn EditorPlugin, &mut PluginContext),
    ) -> Commands {
        let mut commands = Vec::new();
        for plugin in &mut self.plugins {
            if disabled.iter().any(|name| name == plugin.name()) {
                continue;
            }
            let mut context = PluginContext::new(view, &mut commands, plugin.name());
            callback(plugin.as_mut(), &mut context);
        }
        commands
    }
    pub(super) fn change_enabled(
        &mut self,
        view: EditorView<'_>,
        name: &str,
        enabled: bool,
    ) -> Commands {
        self.run(view, &[], |plugin, context| {
            if plugin.name() != name || plugin.core() {
                return;
            }
            plugin.set_enabled(enabled, context);
            if !enabled {
                context.send(EditorCommand::SetLines(Vec::new()));
            }
        })
    }
    pub(super) fn save_settings(&self) -> BTreeMap<String, serde_json::Value> {
        self.plugins
            .iter()
            .filter_map(|plugin| Some((plugin.name().to_owned(), plugin.save_settings()?)))
            .collect()
    }
    pub(super) fn load_settings(&mut self, settings: &BTreeMap<String, serde_json::Value>) {
        for plugin in &mut self.plugins {
            plugin.load_settings(
                settings
                    .get(plugin.name())
                    .unwrap_or(&serde_json::Value::Null),
            );
        }
    }
    pub(super) fn set_lines(&mut self, plugin: &'static str, lines: Vec<LineVertex>) {
        self.lines.insert(plugin, lines);
        self.lines_changed = true;
    }
    pub(super) fn take_lines_update(&mut self) -> Option<Vec<LineVertex>> {
        if !std::mem::take(&mut self.lines_changed) {
            return None;
        }
        Some(self.lines.values().flatten().copied().collect())
    }
}
