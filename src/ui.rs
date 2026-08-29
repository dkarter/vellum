use std::str::FromStr;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
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
    let spacing = usize::from(config.item.spacing.min(list_area.height));
    let capacity = if spacing == 0 {
        app.visible.len()
    } else {
        app.visible.len().saturating_mul(2)
    };
    let mut list_items = Vec::with_capacity(capacity);
    let mut selected_list_index = None;
    for (position, &index) in app.visible.iter().enumerate() {
        if position > 0 && spacing > 0 {
            list_items.push(ListItem::new(vec![Line::default(); spacing]));
        }
        let selected = app.visible.get(app.selected) == Some(&index);
        if selected {
            selected_list_index = Some(list_items.len());
        }
        let alternate_background = (position % 2 == 1)
            .then_some(config.item.alternate_background.as_deref())
            .flatten();
        let lines: Vec<_> = app.items[index]
            .rows
            .iter()
            .map(|row| render_row(row, inner_width, theme, selected, alternate_background))
            .collect();
        let mut list_item = ListItem::new(lines);
        if let Some(background) = alternate_background {
            list_item = list_item.style(Style::new().bg(color(background)));
        }
        list_items.push(list_item);
    }
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
    let mut state = ListState::default().with_selected(selected_list_index);
    frame.render_stateful_widget(list, list_area, &mut state);

    let footer_text = if let Some(status) = &app.status {
        status.clone()
    } else if app.filter_mode {
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
        if config.keybindings.enabled
            && app.has_potential_actions()
            && !config.actions.menu.is_empty()
        {
            text.push_str(&format!("  {} actions", config.actions.menu.label()));
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
        Style::new().fg(if app.status.is_some() {
            Color::Red
        } else {
            color(&theme.border)
        }),
    ));
    frame.render_widget(Paragraph::new(Line::from(footer)), footer_area);

    if app.action_menu {
        render_action_menu(frame, app, config);
    }
}

fn render_action_menu(frame: &mut Frame, app: &App, config: &Config) {
    let area = frame.area();
    let matching = app.matching_action_indices();
    let content_width = matching
        .iter()
        .map(|index| &config.actions.items[*index])
        .map(|action| {
            let heading = if action.icon.is_empty() {
                action.label.clone()
            } else {
                format!("{} {}", action.icon, action.label)
            };
            Line::from(heading)
                .width()
                .max(Line::from(action.description.as_str()).width())
                + if action.key.is_empty() {
                    0
                } else {
                    action.key.label().len() + 3
                }
        })
        .max()
        .unwrap_or(30) as u16;
    let max_width = if area.width >= 12 {
        area.width - 6
    } else {
        area.width
    };
    let width = content_width.saturating_add(4).max(36).min(max_width);
    let action_height = matching.len().saturating_mul(2) as u16;
    let max_height = if area.height >= 8 {
        area.height - 4
    } else {
        area.height
    };
    let height = action_height.saturating_add(4).max(6).min(max_height);
    let popup = centered(area, width, height);
    let outer = Block::new()
        .borders(Borders::ALL)
        .title(" Actions ")
        .style(base_style(&config.theme))
        .border_style(Style::new().fg(color(&config.theme.border)));
    let inner = outer.inner(popup);
    let [search_area, list_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).areas(inner);
    frame.render_widget(Clear, popup);
    frame.render_widget(outer, popup);
    if inner.width < 4 || inner.height < 3 {
        return;
    }

    let (query, cursor_offset) = if app.action_query.is_empty() {
        ("Filter actions...", 0)
    } else {
        search_view(
            &app.action_query,
            app.action_cursor,
            search_area.width.saturating_sub(3) as usize,
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw("› "), Span::raw(query)]))
            .style(base_style(&config.theme))
            .block(
                Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::new().fg(color(&config.theme.border))),
            ),
        search_area,
    );
    frame.set_cursor_position((search_area.x + 2 + cursor_offset, search_area.y));

    let items = matching
        .iter()
        .map(|index| &config.actions.items[*index])
        .map(|action| {
            let mut heading = Vec::new();
            if !action.icon.is_empty() {
                heading.push(Span::styled(
                    format!("{} ", action.icon),
                    Style::new().add_modifier(Modifier::BOLD),
                ));
            }
            heading.push(Span::styled(
                &action.label,
                Style::new().add_modifier(Modifier::BOLD),
            ));
            if !action.key.is_empty() {
                heading.push(Span::styled(
                    format!("  {}", action.key.label()),
                    Style::new().fg(color(&config.theme.border)),
                ));
            }
            ListItem::new(vec![
                Line::from(heading),
                Line::from(Span::styled(
                    &action.description,
                    Style::new().fg(color(&config.theme.border)),
                )),
            ])
        });
    if matching.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.has_pending_actions() {
                "Checking actions..."
            } else {
                "No matching actions"
            })
            .style(Style::new().fg(color(&config.theme.border)))
            .block(Block::new().padding(Padding::horizontal(1))),
            list_area,
        );
        return;
    }
    let list = List::new(items)
        .block(Block::new().padding(Padding::horizontal(1)))
        .highlight_style(
            Style::new()
                .fg(color(&config.theme.selection_foreground))
                .bg(color(&config.theme.selection_background)),
        );
    let mut state = ListState::default().with_selected(Some(app.action_selected));
    frame.render_stateful_widget(list, list_area, &mut state);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
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

fn render_row<'a>(
    row: &'a RenderedRow,
    width: usize,
    theme: &Theme,
    selected: bool,
    background: Option<&str>,
) -> Line<'a> {
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
        .map(|segment| span(segment, theme, selected, background))
        .collect();
    if !right.is_empty() {
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(left_width + right_width)),
        ));
        spans.extend(
            right
                .iter()
                .map(|segment| span(segment, theme, selected, background)),
        );
    }
    Line::from(spans)
}

fn display_width(segment: &RenderedSegment) -> usize {
    Line::from(segment.text.as_str()).width()
}

fn span<'a>(
    segment: &'a RenderedSegment,
    theme: &Theme,
    selected: bool,
    background: Option<&str>,
) -> Span<'a> {
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
        .bg(color(
            segment
                .bg
                .as_deref()
                .or(background)
                .unwrap_or(&theme.background),
        ));
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
    fn ui_010_item_spacing_separates_list_entries() {
        let mut config = Config::parse(
            r##"
                [source]
                cmd = "unused"

                [item]
                spacing = 1
                template = [["$name"]]
                value = "$id"

                [theme]
                selection_background = "#00ffff"
            "##,
        )
        .unwrap();
        config.search.enabled = false;
        let source = json!([
            { "id": "1", "name": "OpenCode" },
            { "id": "2", "name": "Claude" }
        ]);
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
        app.selected = 1;
        let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(1, 0)].symbol(), "O");
        assert_eq!(buffer[(1, 1)].symbol(), " ");
        assert_eq!(buffer[(1, 2)].symbol(), "C");
        assert_ne!(buffer[(1, 1)].bg, buffer[(1, 2)].bg);
    }

    #[test]
    fn ui_011_alternating_backgrounds_follow_visible_order() {
        let mut config = Config::parse(
            r##"
                [source]
                cmd = "unused"

                [item]
                alternate_background = "#202020"
                template = [["$name"]]
                value = "$id"

                [theme]
                background = "#101010"
                selection_background = "#00ffff"
            "##,
        )
        .unwrap();
        config.search.enabled = false;
        let source = json!([
            { "id": "1", "name": "One" },
            { "id": "2", "name": "Two" },
            { "id": "3", "name": "Three" }
        ]);
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
        app.selected = 2;
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(1, 0)].bg, Color::Rgb(16, 16, 16));
        assert_eq!(buffer[(1, 1)].bg, Color::Rgb(32, 32, 32));
        assert_eq!(buffer[(1, 2)].bg, Color::Rgb(0, 255, 255));

        app.visible = vec![1, 2];
        app.selected = 1;
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(1, 0)].bg, Color::Rgb(16, 16, 16));
        assert_eq!(buffer[(1, 1)].bg, Color::Rgb(0, 255, 255));
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

    #[test]
    fn act_009_action_menu_renders_icons_descriptions_and_fuzzy_results() {
        let config = Config::parse(
            r#"
                [source]
                cmd = "unused"

                [actions]
                menu = "ctrl-a"

                [[actions.items]]
                name = "refresh"
                label = "Refresh source"
                icon = "R"
                description = "Rerun the source"
                command = ["true"]

                [[actions.items]]
                name = "failure"
                label = "Show an error"
                icon = "!"
                description = "Display a useful failure"
                command = ["false"]

                [item]
                template = [["$name"]]
                value = "$id"
            "#,
        )
        .unwrap();
        let source = json!([{ "id": "1", "name": "One" }]);
        let mut app = App::new_with_frecency_and_actions(
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
            Default::default(),
            config.actions.clone(),
        );
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        let mut terminal = Terminal::new(TestBackend::new(70, 16)).unwrap();

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
        assert!(output.contains("Actions"), "{output}");
        assert!(output.contains("Filter actions..."), "{output}");
        assert!(output.contains("R Refresh source"), "{output}");
        assert!(output.contains("Rerun the source"), "{output}");
        assert!(output.contains("! Show an error"), "{output}");

        for character in "failure".chars() {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(character),
            ));
        }
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
        assert!(output.contains("failure"), "{output}");
        assert!(output.contains("Show an error"), "{output}");
        assert!(!output.contains("Refresh source"), "{output}");

        let mut tiny = Terminal::new(TestBackend::new(5, 3)).unwrap();
        tiny.draw(|frame| render(frame, &mut app, &config)).unwrap();
    }
}
