use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    config::{Bindings, InputConfig, InputMode, ItemConfig, Keybindings},
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
    input_config: InputConfig,
    search_enabled: bool,
    source_items: Vec<SourceItem>,
    pub items: Vec<RenderedItem>,
    pub visible: Vec<usize>,
    pub query: String,
    pub cursor: usize,
    pub input_mode: InputMode,
    pub selected: usize,
    pub outcome: Outcome,
}

impl App {
    pub fn new(
        source_items: Vec<SourceItem>,
        item_config: ItemConfig,
        keybindings: Keybindings,
        input_config: InputConfig,
        search_enabled: bool,
    ) -> Self {
        let items = render_items(&source_items, &item_config, 0);
        let visible = matching_indices(&items, "");
        Self {
            item_config,
            keybindings,
            search_enabled,
            source_items,
            items,
            visible,
            query: String::new(),
            cursor: 0,
            input_mode: if input_config.vim {
                input_config.start_mode
            } else {
                InputMode::Insert
            },
            input_config,
            selected: 0,
            outcome: Outcome::Running,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&'c'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.outcome = Outcome::Cancelled;
            return;
        }
        if key.code == KeyCode::Esc && self.input_config.vim {
            if self.input_mode == InputMode::Insert {
                self.input_mode = InputMode::Normal;
                self.cursor = previous_boundary(&self.query, self.cursor);
            } else {
                self.outcome = Outcome::Cancelled;
            }
            return;
        }
        if self.bindings_match(key, &self.keybindings.cancel) {
            self.outcome = Outcome::Cancelled;
        } else if self.bindings_match(key, &self.keybindings.accept) {
            if let Some(item) = self.selected_item() {
                self.outcome = Outcome::Accepted(item.value.clone());
            }
        } else if self.bindings_match(key, &self.keybindings.down) {
            self.move_down();
        } else if self.bindings_match(key, &self.keybindings.up) {
            self.move_up();
        } else if self.search_enabled {
            if self.input_mode == InputMode::Normal {
                self.handle_normal_key(key);
            } else {
                self.handle_insert_key(key);
            }
        }
    }

    pub fn vim_enabled(&self) -> bool {
        self.input_config.vim
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

    fn handle_insert_key(&mut self, key: KeyEvent) {
        if self.bindings_match(key, &self.keybindings.forward) {
            self.cursor = next_boundary(&self.query, self.cursor);
        } else if self.bindings_match(key, &self.keybindings.backward) {
            self.cursor = previous_boundary(&self.query, self.cursor);
        } else if self.bindings_match(key, &self.keybindings.start) {
            self.cursor = 0;
        } else if self.bindings_match(key, &self.keybindings.end) {
            self.cursor = self.query.len();
        } else if self.bindings_match(key, &self.keybindings.delete_word) {
            self.delete_previous_word();
        } else {
            match key.code {
                KeyCode::Char(character)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.query.insert(self.cursor, character);
                    self.cursor += character.len_utf8();
                    self.refilter();
                }
                KeyCode::Backspace if self.cursor > 0 => {
                    let start = previous_boundary(&self.query, self.cursor);
                    self.query.replace_range(start..self.cursor, "");
                    self.cursor = start;
                    self.refilter();
                }
                KeyCode::Delete if self.cursor < self.query.len() => {
                    self.delete_at_cursor();
                }
                _ => {}
            }
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if key.modifiers != KeyModifiers::NONE && key.modifiers != KeyModifiers::SHIFT {
            return;
        }
        match key.code {
            KeyCode::Char('i') => self.input_mode = InputMode::Insert,
            KeyCode::Char('a') => {
                self.cursor = next_boundary(&self.query, self.cursor);
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('I') => {
                self.cursor = 0;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('A') => {
                self.cursor = self.query.len();
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.cursor = previous_boundary(&self.query, self.cursor);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                let next = next_boundary(&self.query, self.cursor);
                if next < self.query.len() {
                    self.cursor = next;
                }
            }
            KeyCode::Char('0') | KeyCode::Home => self.cursor = 0,
            KeyCode::Char('$') | KeyCode::End => self.cursor = last_boundary(&self.query),
            KeyCode::Char('b') => self.cursor = previous_word_start(&self.query, self.cursor),
            KeyCode::Char('w') => {
                self.cursor =
                    next_word_start(&self.query, self.cursor).min(last_boundary(&self.query));
            }
            KeyCode::Char('x') | KeyCode::Delete => self.delete_at_cursor(),
            KeyCode::Char('j') => self.move_down(),
            KeyCode::Char('k') => self.move_up(),
            _ => {}
        }
    }

    fn delete_previous_word(&mut self) {
        let start = previous_word_start(&self.query, self.cursor);
        self.query.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.refilter();
    }

    fn delete_at_cursor(&mut self) {
        if self.cursor >= self.query.len() {
            return;
        }
        let end = next_boundary(&self.query, self.cursor);
        self.query.replace_range(self.cursor..end, "");
        if self.input_mode == InputMode::Normal && self.cursor == self.query.len() {
            self.cursor = last_boundary(&self.query);
        }
        self.refilter();
    }

    fn bindings_match(&self, key: KeyEvent, bindings: &Bindings) -> bool {
        self.keybindings.enabled && bindings.matches(key)
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }
}

fn previous_boundary(value: &str, cursor: usize) -> usize {
    value[..cursor]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn next_boundary(value: &str, cursor: usize) -> usize {
    value[cursor..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(value.len(), |(index, _)| cursor + index)
}

fn last_boundary(value: &str) -> usize {
    value
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn previous_word_start(value: &str, cursor: usize) -> usize {
    let graphemes: Vec<_> = value[..cursor].grapheme_indices(true).collect();
    let mut position = graphemes.len();
    while position > 0 && graphemes[position - 1].1.chars().all(char::is_whitespace) {
        position -= 1;
    }
    while position > 0 && !graphemes[position - 1].1.chars().all(char::is_whitespace) {
        position -= 1;
    }
    graphemes.get(position).map_or(0, |(index, _)| *index)
}

fn next_word_start(value: &str, cursor: usize) -> usize {
    let graphemes: Vec<_> = value.grapheme_indices(true).collect();
    let mut position = graphemes.partition_point(|(index, _)| *index < cursor);
    while position < graphemes.len() && !graphemes[position].1.chars().all(char::is_whitespace) {
        position += 1;
    }
    while position < graphemes.len() && graphemes[position].1.chars().all(char::is_whitespace) {
        position += 1;
    }
    graphemes
        .get(position)
        .map_or(value.len(), |(index, _)| *index)
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
            InputConfig::default(),
            true,
        )
    }

    #[test]
    fn sea_002_filters_navigates_and_accepts() {
        let mut app = app();

        app.handle_key(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(app.visible, [1]);
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.outcome, Outcome::Accepted("2".into()));
    }

    #[test]
    fn ref_001_keeps_selected_value_across_refresh() {
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
    fn inp_001_cancel_binding_exits_without_value() {
        let mut app = app();
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.outcome, Outcome::Running);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.outcome, Outcome::Cancelled);
    }

    #[test]
    fn inp_002_nav_001_supports_readline_editing_and_list_bindings() {
        let mut app = app();
        for character in "one two".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(app.query, "one ");
        assert_eq!(app.cursor, 4);

        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 0);
        app.query.clear();
        app.cursor = 0;
        app.refilter();
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn inp_003_can_001_supports_basic_vim_input_and_ctrl_c_always_cancels() {
        let mut app = app();
        for character in "abc".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        app.handle_key(KeyEvent::from(KeyCode::Char('h')));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(app.query, "ac");
        app.handle_key(KeyEvent::from(KeyCode::Char('i')));
        assert_eq!(app.input_mode, InputMode::Insert);

        app.keybindings.enabled = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(app.outcome, Outcome::Cancelled);
    }

    #[test]
    fn inp_004_escape_cancels_immediately_when_vim_mode_is_disabled() {
        let mut app = app();
        app.input_config.vim = false;
        app.input_mode = InputMode::Insert;

        app.handle_key(KeyEvent::from(KeyCode::Esc));

        assert_eq!(app.outcome, Outcome::Cancelled);
    }

    #[test]
    fn inp_005_vim_end_and_delete_keep_cursor_on_the_last_grapheme() {
        let mut app = app();
        for character in "abc".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        app.handle_key(KeyEvent::from(KeyCode::Char('$')));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!((app.query.as_str(), app.cursor), ("ab", 1));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!((app.query.as_str(), app.cursor), ("", 0));
    }

    #[test]
    fn inp_006_editing_treats_combining_and_joined_emoji_as_graphemes() {
        for value in ["e\u{301}", "👨‍👩‍👧‍👦"] {
            let mut app = app();
            for character in value.chars() {
                app.handle_key(KeyEvent::from(KeyCode::Char(character)));
            }

            app.handle_key(KeyEvent::from(KeyCode::Backspace));

            assert_eq!(app.query, "");
            assert_eq!(app.cursor, 0);
        }
    }
}
