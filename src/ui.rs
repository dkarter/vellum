use std::str::FromStr;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::App,
    config::{Alignment, Config, Theme},
    item::{RenderedRow, RenderedSegment},
};

pub fn render(frame: &mut Frame, app: &mut App, config: &Config) {
    let theme = &config.theme;
    let area = frame.area();
    let background = Block::new().style(Style::new().bg(color(&theme.background)));
    frame.render_widget(background, area);

    let (search_area, list_area, footer_area) = if config.search.enabled {
        let [search, list, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);
        (Some(search), list, footer)
    } else {
        let [list, footer] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        (None, list, footer)
    };

    if let Some(search_area) = search_area {
        let (query, cursor_offset) = if app.query.is_empty() {
            (config.search.placeholder.as_str(), 0)
        } else {
            search_view(
                &app.query,
                app.cursor,
                search_area.width.saturating_sub(2) as usize,
            )
        };
        let title = if let Some(choice) = app.active_filter() {
            let filter = if choice.icon.is_empty() {
                choice.label.clone()
            } else {
                format!("{} {}", choice.icon, choice.label)
            };
            let mut style = Style::new().add_modifier(Modifier::BOLD);
            if let Some(fg) = &choice.fg {
                style = style.fg(color(fg));
            }
            Line::from(vec![
                Span::raw(format!(" {}  ", config.search.title)),
                Span::styled(filter, style),
                Span::raw(" "),
            ])
        } else {
            Line::from(format!(" {} ", config.search.title))
        };
        let input = Paragraph::new(query).style(base_style(theme)).block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(color(&theme.border)))
                .title(title),
        );
        frame.render_widget(input, search_area);
        frame.set_cursor_position((search_area.x + 1 + cursor_offset, search_area.y + 1));
    }

    let horizontal_chrome = config
        .item
        .padding
        .saturating_mul(2)
        .saturating_add(if config.item.border { 2 } else { 0 });
    let inner_width = list_area.width.saturating_sub(horizontal_chrome) as usize;
    let list_items: Vec<_> = app
        .visible
        .iter()
        .map(|&index| {
            let selected = app.visible.get(app.selected) == Some(&index);
            let lines: Vec<_> = app.items[index]
                .rows
                .iter()
                .map(|row| render_row(row, inner_width, theme, selected))
                .collect();
            ListItem::new(lines)
        })
        .collect();
    let list_block = Block::new()
        .borders(if config.item.border {
            Borders::LEFT | Borders::RIGHT
        } else {
            Borders::NONE
        })
        .padding(Padding::horizontal(config.item.padding))
        .border_style(Style::new().fg(color(&theme.border)));
    let list = List::new(list_items)
        .block(list_block)
        .highlight_style(Style::new().bg(color(&theme.selection_background)));
    let mut state =
        ListState::default().with_selected((!app.visible.is_empty()).then_some(app.selected));
    frame.render_stateful_widget(list, list_area, &mut state);

    let footer_text = if app.filter_mode {
        let mut keys = Vec::with_capacity(config.filters.choices.len() + 1);
        if !config.filters.clear.is_empty() {
            keys.push(config.filters.clear.label());
        }
        keys.extend(
            config
                .filters
                .choices
                .iter()
                .map(|choice| choice.key.label()),
        );
        format!(
            "{} {}  {}/{} navigate  esc close",
            config.filters.label,
            keys.join("/"),
            config.keybindings.display_binding(&config.keybindings.up),
            config.keybindings.display_binding(&config.keybindings.down),
        )
    } else {
        let mut text = format!(
            "{}/{}  {}/{} navigate  {} select  {} cancel",
            app.visible.len(),
            app.items.len(),
            config.keybindings.display_binding(&config.keybindings.up),
            config.keybindings.display_binding(&config.keybindings.down),
            config
                .keybindings
                .display_binding(&config.keybindings.accept),
            config
                .keybindings
                .display_binding(&config.keybindings.cancel),
        );
        if !config.filters.choices.is_empty() {
            text.push_str(&format!(
                "  {} {}",
                config.filters.mode.label(),
                config.filters.label
            ));
        }
        text
    };
    let mut footer = Vec::with_capacity(3);
    if app.filter_mode {
        footer.push(Span::styled(
            " FILTER ",
            Style::new()
                .fg(color(&theme.selection_foreground))
                .bg(color(&theme.selection_background))
                .add_modifier(Modifier::BOLD),
        ));
        footer.push(Span::raw(" "));
    } else if app.vim_enabled() {
        let (label, background) = match app.input_mode {
            crate::config::InputMode::Insert => (" INSERT ", &theme.insert_mode_background),
            crate::config::InputMode::Normal => (" NORMAL ", &theme.normal_mode_background),
        };
        footer.push(Span::styled(
            label,
            Style::new()
                .fg(color(&theme.mode_foreground))
                .bg(color(background))
                .add_modifier(Modifier::BOLD),
        ));
        footer.push(Span::raw(" "));
    }
    footer.push(Span::styled(
        footer_text,
        Style::new().fg(color(&theme.border)),
    ));
    frame.render_widget(Paragraph::new(Line::from(footer)), footer_area);
}

fn search_view(query: &str, cursor: usize, width: usize) -> (&str, u16) {
    let mut start = 0;
    while start < cursor && Line::from(&query[start..cursor]).width() >= width {
        start = query[start..]
            .grapheme_indices(true)
            .nth(1)
            .map_or(cursor, |(offset, _)| start + offset);
    }
    let cursor_offset = Line::from(&query[start..cursor]).width() as u16;
    (&query[start..], cursor_offset)
}

fn render_row<'a>(row: &'a RenderedRow, width: usize, theme: &Theme, selected: bool) -> Line<'a> {
    let split = row
        .segments
        .iter()
        .position(|segment| segment.align == Alignment::Right);
    let (left, right) = split.map_or((&row.segments[..], &[][..]), |index| {
        row.segments.split_at(index)
    });
    let left_width: usize = left.iter().map(display_width).sum();
    let right_width: usize = right.iter().map(display_width).sum();
    let mut spans: Vec<_> = left
        .iter()
        .map(|segment| span(segment, theme, selected))
        .collect();
    if !right.is_empty() {
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(left_width + right_width)),
        ));
        spans.extend(right.iter().map(|segment| span(segment, theme, selected)));
    }
    Line::from(spans)
}

fn display_width(segment: &RenderedSegment) -> usize {
    Line::from(segment.text.as_str()).width()
}

fn span<'a>(segment: &'a RenderedSegment, theme: &Theme, selected: bool) -> Span<'a> {
    let mut style = Style::new()
        .fg(segment.fg.as_deref().map_or_else(
            || {
                color(if selected {
                    &theme.selection_foreground
                } else {
                    &theme.foreground
                })
            },
            color,
        ))
        .bg(segment
            .bg
            .as_deref()
            .map_or_else(|| color(&theme.background), color));
    if segment.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Span::styled(segment.text.as_str(), style)
}

fn base_style(theme: &Theme) -> Style {
    Style::new()
        .fg(color(&theme.foreground))
        .bg(color(&theme.background))
}

fn color(value: &str) -> Color {
    Color::from_str(value).unwrap_or(Color::Reset)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    use super::*;

    #[test]
    fn ui_001_renders_search_multiline_items_and_footer() {
        let config = Config::parse(
            r##"
                [source]
                cmd = "unused"

                [item]
                template = [["$name", { token = "$status", align = "right" }], ["$detail"]]
                value = "$id"

                [theme]
                selection_background = "#00ffff"
            "##,
        )
        .unwrap();
        let source = json!([{ "id": "1", "name": "OpenCode", "status": "running", "detail": "Implementing feature" }]);
        let mut app = App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        let backend = TestBackend::new(50, 9);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

        assert!(output.contains("Vellum"));
        assert!(output.contains("OpenCode"));
        assert!(output.contains("running"));
        assert!(output.contains("Implementing feature"));
        assert!(output.contains("1/1"));
        assert_ne!(buffer[(0, 3)].symbol(), "│");
        assert_ne!(buffer[(49, 3)].symbol(), "│");
        assert_eq!(buffer[(1, 3)].symbol(), "O");
    }

    #[test]
    fn ui_002_renders_item_border_when_enabled() {
        let mut config = Config::parse(
            r#"
                [source]
                cmd = "unused"

                [item]
                border = true
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        config.search.enabled = false;
        let source = json!([{ "id": "1", "name": "OpenCode" }]);
        let mut app = App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            false,
        );
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();

        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "│");
        assert_eq!(terminal.backend().buffer()[(19, 0)].symbol(), "│");
    }

    #[test]
    fn ui_003_applies_configurable_item_padding() {
        let mut config = Config::parse(
            r#"
                [source]
                cmd = "unused"

                [item]
                padding = 3
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        config.search.enabled = false;
        let source = json!([{ "id": "1", "name": "OpenCode" }]);
        let mut app = App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            false,
        );
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();

        assert_eq!(terminal.backend().buffer()[(2, 0)].symbol(), " ");
        assert_eq!(terminal.backend().buffer()[(3, 0)].symbol(), "O");
    }

    #[test]
    fn ui_004_large_padding_saturates_on_narrow_terminals() {
        let mut config = Config::parse(
            r#"
                [source]
                cmd = "unused"

                [item]
                padding = 65535
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        config.search.enabled = false;
        let source = json!([{ "id": "1", "name": "OpenCode" }]);
        let mut app = App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            false,
        );
        let backend = TestBackend::new(10, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
    }

    #[test]
    fn ui_005_search_view_keeps_long_query_cursor_inside_input() {
        let (view, offset) = search_view("abcdefgh", 8, 4);
        assert_eq!(view, "fgh");
        assert_eq!(offset, 3);

        let query = "界界界";
        let (view, offset) = search_view(query, query.len(), 4);
        assert_eq!(view, "界");
        assert_eq!(offset, 2);
    }

    #[test]
    fn ui_007_vim_mode_badge_reflects_input_state() {
        let mut config = Config::parse(
            r#"
                [search]
                title = "Files"

                [source]
                cmd = "unused"

                [item]
                template = [["$name"]]
                value = "$id"

                [theme]
                insert_mode_background = "green"
            "#,
        )
        .unwrap();
        let source = json!([{ "id": "1", "name": "main.rs" }]);
        let source_items = source
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_object().unwrap().clone())
            .collect::<Vec<_>>();
        let backend = TestBackend::new(40, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(
            source_items.clone(),
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(output.contains("Files"));
        assert!(output.contains("INSERT"));
        assert_eq!(buffer[(0, 5)].bg, Color::Green);

        app.input_mode = crate::config::InputMode::Normal;
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(output.contains("NORMAL"));
        assert_eq!(buffer[(0, 5)].bg, Color::Yellow);

        config.input.vim = false;
        let mut app = App::new(
            source_items,
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!output.contains("INSERT"));
        assert!(!output.contains("NORMAL"));
    }

    #[test]
    fn ui_009_footer_reflects_filter_state() {
        let config = Config::parse(
            r#"
                [search]
                title = "Agents"

                [source]
                cmd = "unused"

                [filters]
                label = "state"
                mode = "ctrl-g"

                [[filters.choices]]
                key = "w"
                label = "working"
                source = "state"
                value = "working"
                icon = "●"
                fg = "blue"

                [item]
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        let source = json!([{ "id": "1", "name": "OpenCode", "state": "working" }]);
        let mut app = App::new(
            source
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_object().unwrap().clone())
                .collect(),
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 6)).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(output.contains("ctrl-g state"));

        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('w'),
        ));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(output.contains("Agents  ● working"), "{output}");
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .find(|cell| cell.symbol() == "●")
                .unwrap()
                .fg,
            Color::Blue
        );

        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(output.contains("FILTER"));
        assert!(output.contains("state a/w"));
    }
}
