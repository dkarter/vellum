use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    config::{ItemConfig, Keybindings},
    item::{RenderedItem, matching_indices, render_items},
    source::SourceItem,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Accepted(String),
    Cancelled,
}

#[derive(Debug)]
pub struct App {
    item_config: ItemConfig,
    keybindings: Keybindings,
    search_enabled: bool,
    source_items: Vec<SourceItem>,
    pub items: Vec<RenderedItem>,
    pub visible: Vec<usize>,
    pub query: String,
    pub selected: usize,
    pub outcome: Outcome,
}

impl App {
    pub fn new(
        source_items: Vec<SourceItem>,
        item_config: ItemConfig,
        keybindings: Keybindings,
        search_enabled: bool,
    ) -> Self {
        let items = render_items(&source_items, &item_config, 0);
        let visible = matching_indices(&items, "");
        Self {
            item_config,
            keybindings: keybindings.normalized(),
            search_enabled,
            source_items,
            items,
            visible,
            query: String::new(),
            selected: 0,
            outcome: Outcome::Running,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if binding_matches(key, &self.keybindings.cancel) {
            self.outcome = Outcome::Cancelled;
        } else if binding_matches(key, &self.keybindings.accept) {
            if let Some(item) = self.selected_item() {
                self.outcome = Outcome::Accepted(item.value.clone());
            }
        } else if binding_matches(key, &self.keybindings.down) {
            self.move_down();
        } else if binding_matches(key, &self.keybindings.up) {
            self.move_up();
        } else if self.search_enabled {
            match key.code {
                KeyCode::Char(character)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.query.push(character);
                    self.refilter();
                }
                KeyCode::Backspace => {
                    self.query.pop();
                    self.refilter();
                }
                _ => {}
            }
        }
    }

    pub fn tick(&mut self, elapsed_ms: u64) {
        self.items = render_items(&self.source_items, &self.item_config, elapsed_ms);
        self.visible = matching_indices(&self.items, &self.query);
        self.clamp_selection();
    }

    pub fn animation_interval(&self) -> Option<std::time::Duration> {
        self.item_config
            .tokens
            .iter()
            .filter_map(|token| token.animation_fps)
            .max()
            .map(|fps| std::time::Duration::from_millis(1_000 / u64::from(fps)))
    }

    pub fn replace_source(&mut self, source_items: Vec<SourceItem>, elapsed_ms: u64) -> bool {
        if self.source_items == source_items {
            return false;
        }
        let selected_value = self.selected_item().map(|item| item.value.clone());
        self.source_items = source_items;
        self.items = render_items(&self.source_items, &self.item_config, elapsed_ms);
        self.visible = matching_indices(&self.items, &self.query);
        self.selected = selected_value
            .and_then(|value| {
                self.visible
                    .iter()
                    .position(|&index| self.items[index].value == value)
            })
            .unwrap_or(0);
        self.clamp_selection();
        true
    }

    pub fn selected_item(&self) -> Option<&RenderedItem> {
        self.visible
            .get(self.selected)
            .and_then(|&index| self.items.get(index))
    }

    fn move_down(&mut self) {
        if !self.visible.is_empty() {
            self.selected = (self.selected + 1).min(self.visible.len() - 1);
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn refilter(&mut self) {
        self.visible = matching_indices(&self.items, &self.query);
        self.selected = 0;
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }
}

fn binding_matches(key: KeyEvent, binding: &str) -> bool {
    let (modifiers, name) = binding
        .strip_prefix("ctrl-")
        .map_or((KeyModifiers::NONE, binding), |name| {
            (KeyModifiers::CONTROL, name)
        });
    if key.modifiers != modifiers {
        return false;
    }
    match name {
        "up" => key.code == KeyCode::Up,
        "down" => key.code == KeyCode::Down,
        "left" => key.code == KeyCode::Left,
        "right" => key.code == KeyCode::Right,
        "enter" => key.code == KeyCode::Enter,
        "esc" | "escape" => key.code == KeyCode::Esc,
        "backspace" => key.code == KeyCode::Backspace,
        value => {
            let mut chars = value.chars();
            matches!((chars.next(), chars.next()), (Some(expected), None) if key.code == KeyCode::Char(expected))
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyEvent;
    use serde_json::json;

    use super::*;

    fn app() -> App {
        let config: ItemConfig = toml::from_str(
            r#"
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        let source = json!([
            { "id": "1", "name": "Alpha" },
            { "id": "2", "name": "Beta" }
        ]);
        App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config,
            Keybindings::default(),
            true,
        )
    }

    #[test]
    fn filters_navigates_and_accepts() {
        let mut app = app();

        app.handle_key(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(app.visible, [1]);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.outcome, Outcome::Accepted("2".into()));
    }

    #[test]
    fn keeps_selected_value_across_refresh() {
        let mut app = app();
        app.handle_key(KeyEvent::from(KeyCode::Down));
        let replacement = json!([
            { "id": "2", "name": "Beta updated" },
            { "id": "1", "name": "Alpha" }
        ]);

        app.replace_source(
            replacement
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            0,
        );

        assert_eq!(app.selected_item().unwrap().value, "2");
    }

    #[test]
    fn cancel_binding_exits_without_value() {
        let mut app = app();
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.outcome, Outcome::Cancelled);
    }
}
