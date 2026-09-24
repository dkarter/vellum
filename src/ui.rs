use std::str::FromStr;

use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Alignment as LayoutAlignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::App,
    config::{Alignment, Config, PreviewBorder, PreviewPosition, Theme},
    item::{RenderedRow, RenderedSegment},
};

pub fn render(frame: &mut Frame, app: &mut App, config: &Config) {
    if let Some(position) = render_with_cursor_position(frame, app, config) {
        frame.set_cursor_position(position);
    }
}

/// Redraws the UI without exposing Ratatui's terminal drawing cursor.
///
/// Ratatui moves the terminal cursor through changed cells while painting. Positioning
/// the hidden cursor before showing it prevents cursor-motion shaders from animating
/// those implementation-detail movements.
pub fn redraw<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    config: &Config,
) -> Result<(), B::Error> {
    terminal.hide_cursor()?;
    let mut cursor_position = None;
    terminal.draw(|frame| {
        cursor_position = render_with_cursor_position(frame, app, config);
    })?;
    if let Some(position) = cursor_position {
        terminal.set_cursor_position(position)?;
        terminal.show_cursor()?;
    }
    Ok(())
}

fn render_with_cursor_position(
    frame: &mut Frame,
    app: &mut App,
    config: &Config,
) -> Option<(u16, u16)> {
    let theme = &config.theme;
    let mut cursor_position = None;
    let area = frame.area();
    app.preview_visible = false;
    let background = Block::new().style(Style::new().bg(color(&theme.background)));
    frame.render_widget(background, area);

    let (search_area, mut list_area, footer_area) = if config.search.enabled {
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
    app.preview_area = None;

    if config.preview.enabled && list_area.width >= 42 && list_area.height >= 8 {
        let position = config.preview.position;
        let percent = config.preview.size;
        let side_width =
            (list_area.width.saturating_mul(percent) / 100).min(list_area.width.saturating_sub(30));
        let (list, preview) = match position {
            PreviewPosition::Left => {
                let [preview, list] =
                    Layout::horizontal([Constraint::Length(side_width), Constraint::Fill(1)])
                        .areas(list_area);
                (list, preview)
            }
            PreviewPosition::Right => {
                let [list, preview] =
                    Layout::horizontal([Constraint::Fill(1), Constraint::Length(side_width)])
                        .areas(list_area);
                (list, preview)
            }
            PreviewPosition::Top => {
                let [preview, list] =
                    Layout::vertical([Constraint::Percentage(percent), Constraint::Fill(1)])
                        .areas(list_area);
                (list, preview)
            }
            PreviewPosition::Bottom => {
                let [list, preview] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Percentage(percent)])
                        .areas(list_area);
                (list, preview)
            }
        };
        if list.width >= 18 && list.height >= 3 && preview.width >= 18 && preview.height >= 3 {
            app.preview_visible = true;
            app.preview_area = Some(preview);
            list_area = list;
            render_preview(frame, app, config, preview);
        }
    }
    app.results_area = list_area;

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
        let mut block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(color(&theme.border)))
            .title(format!(" {} ", config.search.title));
        if let Some(title) = filter_title(app, config) {
            block = block.title(title.alignment(LayoutAlignment::Right));
        }
        let input = Paragraph::new(query).style(base_style(theme)).block(block);
        frame.render_widget(input, search_area);
        cursor_position = Some((search_area.x + 1 + cursor_offset, search_area.y + 1));
    }

    let horizontal_chrome = config
        .item
        .padding
        .saturating_mul(2)
        .saturating_add(if config.item.border { 2 } else { 0 });
    let boxed = config.item.box_title.is_some();
    let inner_width = list_area
        .width
        .saturating_sub(horizontal_chrome.saturating_add(if boxed { 2 } else { 0 }))
        as usize;
    let list_height = list_area.height.saturating_sub(if boxed { 2 } else { 0 });
    let spacing = usize::from(config.item.spacing.min(list_height));
    let item_height = app
        .visible
        .first()
        .and_then(|index| app.items.get(*index))
        .map(|item| item.rows.len())
        .unwrap_or(1)
        .max(1);
    let page_size = (usize::from(list_height).saturating_add(spacing)
        / item_height.saturating_add(spacing))
    .max(1);
    app.set_list_page_size(page_size);
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
    let mut list_block = Block::new()
        .borders(if boxed {
            Borders::ALL
        } else if config.item.border {
            Borders::LEFT | Borders::RIGHT
        } else {
            Borders::NONE
        })
        .padding(Padding::horizontal(config.item.padding))
        .border_style(Style::new().fg(color(&theme.border)));
    if let Some(title) = &config.item.box_title {
        list_block = list_block.title(format!(" {title} "));
    }
    let list = List::new(list_items)
        .block(list_block)
        .highlight_style(Style::new().bg(color(&theme.selection_background)));
    let mut state = ListState::default().with_selected(selected_list_index);
    frame.render_stateful_widget(list, list_area, &mut state);

    let mut footer = Vec::new();
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
    if let Some(status) = &app.status {
        footer.push(Span::styled(
            status.clone(),
            Style::new().fg(if app.status_is_error() {
                Color::Red
            } else {
                color(&theme.border)
            }),
        ));
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
        push_footer_hint(
            &mut footer,
            "",
            keys.join("/"),
            &config.filters.label,
            theme,
        );
        push_footer_hint(&mut footer, "  ", "tab/shift-tab", "cycle", theme);
        push_footer_hint(&mut footer, "  ", "esc", "close", theme);
    } else {
        footer.push(Span::styled(
            format!("{}/{}", app.visible.len(), app.items.len()),
            Style::new().fg(color(&theme.border)),
        ));
        push_footer_hint(
            &mut footer,
            "  ",
            format!(
                "{}/{}",
                config.keybindings.display_binding(&config.keybindings.up),
                config.keybindings.display_binding(&config.keybindings.down),
            ),
            "navigate",
            theme,
        );
        push_footer_hint(
            &mut footer,
            "  ",
            config
                .keybindings
                .display_binding(&config.keybindings.accept),
            "select",
            theme,
        );
        push_footer_hint(
            &mut footer,
            "  ",
            config
                .keybindings
                .display_binding(&config.keybindings.cancel),
            "cancel",
            theme,
        );
        if !config.filters.choices.is_empty() {
            push_footer_hint(
                &mut footer,
                "  ",
                config.filters.mode.label(),
                &config.filters.label,
                theme,
            );
        }
        if config.keybindings.enabled
            && app.has_potential_actions()
            && !config.actions.menu.is_empty()
        {
            push_footer_hint(
                &mut footer,
                "  ",
                config.actions.menu.label(),
                "actions",
                theme,
            );
        }
        if app.preview_visible {
            push_footer_hint(
                &mut footer,
                "  ",
                format!(
                    "{}/{}",
                    config.preview.scroll_up.label(),
                    config.preview.scroll_down.label(),
                ),
                "preview",
                theme,
            );
        }
    }
    frame.render_widget(Paragraph::new(Line::from(footer)), footer_area);

    if app.action_menu
        && let Some(position) = render_action_menu(frame, app, config)
    {
        cursor_position = Some(position);
    }
    cursor_position
}

fn push_footer_hint(
    spans: &mut Vec<Span<'static>>,
    separator: &'static str,
    keys: impl Into<String>,
    label: &str,
    theme: &Theme,
) {
    spans.push(Span::raw(separator));
    spans.push(Span::styled(
        keys.into(),
        Style::new().fg(color(&theme.foreground)),
    ));
    spans.push(Span::styled(
        format!(" {label}"),
        Style::new().fg(color(&theme.border)),
    ));
}

fn filter_title(app: &App, config: &Config) -> Option<Line<'static>> {
    if config.filters.choices.is_empty() {
        return None;
    }

    let theme = &config.theme;
    let selected = app.active_filter_index();
    let expansion = app.filter_expansion();
    if selected.is_none() && expansion <= 0.0 {
        return None;
    }
    let mut labels = Vec::with_capacity(config.filters.choices.len() + 1);
    labels.push(if selected.is_none() {
        config.filters.all_label.clone()
    } else {
        config.filters.clear.label().to_owned()
    });
    for (index, choice) in config.filters.choices.iter().enumerate() {
        labels.push(if selected == Some(index) {
            choice.label.clone()
        } else {
            choice.key.label().to_owned()
        });
    }

    let (start, end) = app.filter_highlight();
    let highlight_start = start.round() as usize;
    let highlight_end = end.round() as usize;
    let highlight = selected
        .and_then(|index| config.filters.choices[index].fg.as_deref())
        .map(color)
        .unwrap_or_else(|| color(&theme.selection_background));
    let foreground = color(&theme.foreground);
    let background = color(&theme.background);
    let text_on_highlight = if selected.is_none() {
        color(&theme.selection_foreground)
    } else {
        background
    };

    let mut spans = Vec::new();
    let mut cell = 0;
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            push_title_cells(
                &mut spans,
                &config.filters.separator,
                &mut cell,
                highlight_start..highlight_end,
                highlight,
                color(&theme.border),
                text_on_highlight,
            );
        }
        push_title_cells(
            &mut spans,
            &format!(" {label} "),
            &mut cell,
            highlight_start..highlight_end,
            highlight,
            if app.filter_has_items(index.checked_sub(1)) {
                foreground
            } else {
                color(&theme.border)
            },
            text_on_highlight,
        );
    }
    let (choice_start, choice_end) = app.filter_bounds(selected, selected);
    let choice_start = choice_start as usize;
    let choice_end = choice_end as usize;
    let visible = if selected.is_none() {
        let keep = (cell as f32 * expansion).round() as usize;
        cell.saturating_sub(keep)..cell
    } else {
        0..cell
    };
    let ranges = if selected.is_none() {
        vec![visible]
    } else {
        let before = (choice_start as f32 * expansion).round() as usize;
        let after = ((cell - choice_end) as f32 * expansion).round() as usize;
        vec![
            choice_start - before..choice_start,
            choice_start..choice_end,
            choice_end..choice_end + after,
        ]
    };
    let clipped = clip_filter_spans(spans, &ranges);
    Some(Line::from(clipped))
}

fn clip_filter_spans(
    spans: Vec<Span<'static>>,
    ranges: &[std::ops::Range<usize>],
) -> Vec<Span<'static>> {
    let mut clipped: Vec<Span<'static>> = Vec::new();
    let mut offset = 0;
    for span in spans {
        for grapheme in span.content.graphemes(true) {
            let width = Line::from(grapheme).width();
            if ranges
                .iter()
                .any(|range| offset >= range.start && offset + width <= range.end)
            {
                if let Some(last) = clipped.last_mut()
                    && last.style == span.style
                {
                    last.content.to_mut().push_str(grapheme);
                } else {
                    clipped.push(Span::styled(grapheme.to_owned(), span.style));
                }
            }
            offset += width;
        }
    }
    clipped
}

fn push_title_cells(
    spans: &mut Vec<Span<'static>>,
    text: &str,
    offset: &mut usize,
    highlight: std::ops::Range<usize>,
    background: Color,
    foreground: Color,
    text_on_highlight: Color,
) {
    for grapheme in text.graphemes(true) {
        let width = Line::from(grapheme).width();
        let mut style = Style::new().fg(foreground);
        if *offset < highlight.end && *offset + width > highlight.start {
            style = style
                .bg(background)
                .fg(text_on_highlight)
                .add_modifier(Modifier::BOLD);
        }
        if let Some(last) = spans.last_mut()
            && last.style == style
        {
            last.content.to_mut().push_str(grapheme);
        } else {
            spans.push(Span::styled(grapheme.to_owned(), style));
        }
        *offset += width;
    }
}

fn render_preview(frame: &mut Frame, app: &mut App, config: &Config, area: Rect) {
    let theme = &config.theme;
    let borders = match config.preview.border {
        PreviewBorder::None => Borders::NONE,
        PreviewBorder::Full => Borders::ALL,
        PreviewBorder::Separator => match config.preview.position {
            PreviewPosition::Left => Borders::RIGHT,
            PreviewPosition::Right => Borders::LEFT,
            PreviewPosition::Top => Borders::BOTTOM,
            PreviewPosition::Bottom => Borders::TOP,
        },
    };
    let block = Block::new()
        .borders(borders)
        .border_style(Style::new().fg(color(&theme.border)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut content = inner;
    let has_title = !config.preview.title.is_empty() && content.height > 1;
    let preview_height = content.height.saturating_sub(u16::from(has_title));
    app.set_preview_height(preview_height as usize);
    if has_title {
        let heading = Line::from(vec![
            Span::styled(
                " ◈ ",
                Style::new()
                    .fg(color(&theme.selection_background))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &config.preview.title,
                Style::new()
                    .fg(color(&theme.foreground))
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(
            heading,
            Rect {
                height: 1,
                ..content
            },
        );
        if content.width >= 28 && app.preview_lines.lines.len() > preview_height as usize {
            let percent = app.preview_offset.saturating_mul(100) / app.preview_max_offset().max(1);
            let indicator = format!(" {percent:>3}% ");
            frame.render_widget(
                Paragraph::new(indicator).style(Style::new().fg(color(&theme.border))),
                Rect {
                    x: content.right() - 6,
                    width: 6,
                    height: 1,
                    ..content
                },
            );
        }
        content.y += 1;
        content.height -= 1;
    }
    if config.source.builtin == Some(crate::builtins::BuiltinSource::Themes) {
        render_theme_showcase(frame, content, app, theme);
        return;
    }
    let count = app.preview_lines.lines.len();
    let show_scrollbar =
        config.preview.scrollbar && count > content.height as usize && content.width > 2;
    if show_scrollbar {
        let scrollbar_area = Rect {
            x: content.right() - 1,
            width: 1,
            ..content
        };
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .thumb_symbol("▏")
            .track_symbol(Some("┊"))
            .thumb_style(Style::new().fg(color(&theme.selection_background)))
            .track_style(Style::new().fg(color(&theme.border)));
        let mut state = ScrollbarState::new(count.saturating_sub(content.height as usize) + 1)
            .position(app.preview_offset);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
        content.width -= 1;
    }
    frame.render_widget(Paragraph::new("").style(base_style(theme)), content);
    for (row, line) in app.preview_lines.lines[app.preview_offset.min(count)..]
        .iter()
        .take(content.height as usize)
        .enumerate()
    {
        render_preview_line(
            frame,
            line,
            Rect {
                y: content.y + row as u16,
                height: 1,
                ..content
            },
        );
    }
}

/// Clip while traversing spans, so a long unbroken ANSI line never needs a
/// full-width measurement (or a clone) on every redraw.
fn render_preview_line(frame: &mut Frame, line: &Line<'_>, mut area: Rect) {
    for span in &line.spans {
        if area.width == 0 {
            break;
        }
        let mut width = 0;
        let mut end = 0;
        for (index, grapheme) in span.content.grapheme_indices(true) {
            let grapheme_width = Line::from(grapheme).width();
            if width + grapheme_width > area.width as usize {
                break;
            }
            width += grapheme_width;
            end = index + grapheme.len();
        }
        if end > 0 {
            frame.render_widget(Span::styled(&span.content[..end], span.style), area);
            area.x += width as u16;
            area.width -= width as u16;
        }
    }
}

fn render_theme_showcase(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width < 16 || area.height < 6 {
        return;
    }
    let swatch_height = if area.height >= 10 { 2 } else { 1 };
    let [search, results, swatches] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(swatch_height),
    ])
    .areas(area);
    let search_block = Block::new()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(color(&theme.border)))
        .title(" Example palette ");
    frame.render_widget(
        Paragraph::new("Search workspaces...")
            .style(base_style(theme))
            .block(search_block),
        search,
    );
    let selected = Style::new()
        .fg(color(&theme.selection_foreground))
        .bg(color(&theme.selection_background))
        .add_modifier(Modifier::BOLD);
    let items = if area.width < 42 {
        [
            ListItem::new("  ◇  dotfiles"),
            ListItem::new("  ◆  vellum"),
            ListItem::new("  ◇  notes"),
        ]
    } else {
        [
            ListItem::new("  ◇  dotfiles       ~/dotfiles"),
            ListItem::new("  ◆  vellum         ~/dev/vellum"),
            ListItem::new("  ◇  notes          ~/notes"),
        ]
    };
    let list = List::new(items).highlight_style(selected);
    let mut state = ListState::default().with_selected(Some(1));
    frame.render_stateful_widget(list, results, &mut state);
    let name = app
        .selected_source_item()
        .and_then(|item| item.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Theme");
    let swatches_line = Line::from(vec![
        Span::styled(" ▪▪ ", Style::new().fg(color(&theme.foreground))),
        Span::styled(" ▪▪ ", Style::new().fg(color(&theme.selection_background))),
        Span::styled(
            " ▪▪ ",
            Style::new().fg(color(&theme.insert_mode_background)),
        ),
        Span::styled(
            " ▪▪ ",
            Style::new().fg(color(&theme.normal_mode_background)),
        ),
    ]);
    if swatch_height == 2 {
        frame.render_widget(
            Paragraph::new(name).style(Style::new().fg(color(&theme.border))),
            Rect {
                height: 1,
                ..swatches
            },
        );
    }
    frame.render_widget(
        swatches_line,
        Rect {
            y: swatches.bottom() - 1,
            height: 1,
            ..swatches
        },
    );
}

fn render_action_menu(frame: &mut Frame, app: &App, config: &Config) -> Option<(u16, u16)> {
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
        return None;
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
    let cursor_position = (search_area.x + 2 + cursor_offset, search_area.y);

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
        return Some(cursor_position);
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
    Some(cursor_position)
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    use super::*;

    #[test]
    fn ui_020_large_ansi_preview_starts_at_bottom_and_scrolls_normally() {
        let config = Config::parse("[source]\ncmd='unused'\n[item]\ntemplate=[['$name']]\nvalue='$name'\n[preview]\nenabled=true\ninitial_scroll='bottom'\ncommand=['cat','$name']").unwrap();
        let mut app = App::new(
            vec![json!({"name":"One"}).as_object().unwrap().clone()],
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        app.configure_preview(config.preview.clone());
        let content = std::sync::Arc::new(crate::preview::parse_ansi(
            &(0..3000)
                .map(|i| format!("\u{1b}[31mrow-{i:04}\u{1b}[0m\n"))
                .collect::<String>(),
        ));
        app.set_preview_content(content.clone());
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(app.preview_offset, app.preview_max_offset());
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == "9" && cell.fg == Color::Indexed(1))
        );
        let rendered: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(rendered.contains("row-2999"));
        assert!(!rendered.contains("row-0000"));

        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(app.preview_offset < app.preview_max_offset());
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!rendered.contains("row-2999"));
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_offset, app.preview_max_offset());

        app.set_preview_content(content); // cached result starts at its configured edge too
        terminal.resize(Rect::new(0, 0, 80, 16)).unwrap();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(app.preview_offset, app.preview_max_offset());
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("row-2999"));

        let mut default_config = config.clone();
        default_config.preview.initial_scroll = Default::default();
        app.configure_preview(default_config.preview);
        app.set_preview_text("first\nlast".into());
        assert_eq!(app.preview_offset, 0);
    }

    #[test]
    fn ui_020_long_ansi_line_clips_to_the_viewport() {
        let config = Config::parse("[source]\ncmd='unused'\n[item]\ntemplate=[['$name']]\nvalue='$name'\n[preview]\nenabled=true\ninitial_scroll='bottom'\ncommand=['true']").unwrap();
        let mut app = App::new(
            vec![json!({"name":"One"}).as_object().unwrap().clone()],
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        app.configure_preview(config.preview.clone());
        app.set_preview_text(format!("\u{1b}[31m{}\u{1b}[0m", "x".repeat(128 * 1024)));
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let area = app.preview_area.unwrap();
        assert_eq!(buffer[(area.x + 1, area.y + 1)].symbol(), "x");
        assert_eq!(buffer[(area.x + 1, area.y + 1)].fg, Color::Indexed(1));
        assert_eq!(buffer[(area.right() - 2, area.y + 1)].symbol(), "x");
    }

    #[test]
    fn ui_018_preview_chrome_and_named_results_box_render() {
        let mut config = Config::parse("[source]\ncmd='unused'\n[item]\nbox_title='Results'\ntemplate=[['$name']]\nvalue='$name'\n[preview]\nenabled=true\ncommand=['cat','$name']").unwrap();
        let mut app = App::new(
            vec![json!({"name":"One"}).as_object().unwrap().clone()],
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        app.set_preview_text((0..60).map(|i| format!("Line {i}\n")).collect());
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let output: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(output.contains("Results"));
        assert!(output.contains("▏"));
        assert!(output.contains("◈ Preview"));
        assert_eq!(buffer[(0, 3)].symbol(), "┌");
        config.preview.border = PreviewBorder::Full;
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .filter(|cell| cell.symbol() == "┌")
                .count()
                >= 2
        );
        config.preview.border = PreviewBorder::None;
        config.preview.scrollbar = false;
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
        assert!(!output.contains("▏"));
    }

    #[test]
    fn ui_016_preview_layout_adapts_to_available_space() {
        let mut config = Config::parse("[source]\ncmd='unused'\n[item]\ntemplate=[['$name']]\nvalue='$name'\n[preview]\nenabled=true\ncommand=['cat','$name']").unwrap();
        let mut app = App::new(
            vec![json!({"name":"Result"}).as_object().unwrap().clone()],
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        app.set_preview_text("Preview content".into());
        for position in [
            PreviewPosition::Left,
            PreviewPosition::Right,
            PreviewPosition::Top,
            PreviewPosition::Bottom,
        ] {
            config.preview.position = position;
            let mut terminal = Terminal::new(TestBackend::new(80, 22)).unwrap();
            terminal
                .draw(|frame| render(frame, &mut app, &config))
                .unwrap();
            let buffer = terminal.backend().buffer();
            let result = buffer
                .content
                .iter()
                .position(|cell| cell.symbol() == "R")
                .unwrap();
            let preview = buffer
                .content
                .iter()
                .position(|cell| cell.symbol() == "P" && cell.fg == color(&config.theme.foreground))
                .unwrap();
            match position {
                PreviewPosition::Left => assert!(preview % 80 < result % 80),
                PreviewPosition::Right => assert!(preview % 80 > result % 80),
                PreviewPosition::Top => assert!(preview / 80 < result / 80),
                PreviewPosition::Bottom => assert!(preview / 80 > result / 80),
            }
            let mut tiny = Terminal::new(TestBackend::new(20, 5)).unwrap();
            tiny.draw(|frame| render(frame, &mut app, &config)).unwrap();
            let output: String = tiny
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(output.contains("Result"));
            assert!(!output.contains("Preview content"));
        }
    }

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
            r##"
                [search]
                title = "Agents"

                [source]
                cmd = "unused"

                [theme]
                background = "#1a1b26"
                selection_background = "#283457"
                selection_foreground = "#c0caf5"

                [filters]
                label = "state"
                all_label = "everyone"
                separator = "·"
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
            "##,
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
        let footer_text: String = (0..80)
            .map(|x| terminal.backend().buffer()[(x, 5)].symbol())
            .collect();
        let hint = footer_text.find("ctrl-g state").unwrap();
        assert_eq!(
            terminal.backend().buffer()[(hint as u16, 5)].fg,
            color(&config.theme.foreground)
        );
        assert_eq!(
            terminal.backend().buffer()[((hint + 7) as u16, 5)].fg,
            color(&config.theme.border)
        );
        assert!(!output.contains("everyone"), "{output}");

        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        app.settle_filter_animation();
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
        assert!(output.contains("everyone · w"), "{output}");
        assert!(output.contains("a/w state"), "{output}");
        assert_eq!(
            terminal.backend().buffer()[(9, 5)].fg,
            color(&config.theme.foreground)
        );
        assert_eq!(
            terminal.backend().buffer()[(13, 5)].fg,
            color(&config.theme.border)
        );
        let all = (0..80)
            .map(|x| &terminal.backend().buffer()[(x, 0)])
            .find(|cell| {
                cell.symbol() == "e" && cell.bg == color(&config.theme.selection_background)
            })
            .unwrap();
        assert_eq!(all.fg, color(&config.theme.selection_foreground));
        let separator = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .find(|cell| cell.symbol() == "·")
            .unwrap();
        assert_eq!(separator.fg, color(&config.theme.border));
        let inactive = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .find(|cell| cell.symbol() == "w")
            .unwrap();
        assert_eq!(inactive.fg, color(&config.theme.foreground));

        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('w'),
        ));
        app.settle_filter_animation();
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
        assert!(output.contains("a · working"), "{output}");
        assert_eq!(terminal.backend().buffer()[(75, 0)].bg, Color::Blue);
        assert!(app.animation_interval().is_none());

        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        assert!(app.animation_interval().is_some());
        app.settle_filter_animation();
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
        assert!(!output.contains("FILTER"));
        assert!(output.contains("working"));
        assert!(!output.contains("·"));
        assert_eq!(terminal.backend().buffer()[(75, 0)].bg, Color::Blue);

        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('a'),
        ));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        app.settle_filter_animation();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        let top: String = (0..80)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect();
        assert!(
            !top.contains("everyone") && !top.contains("working"),
            "{top}"
        );
    }

    #[test]
    fn ui_009_filter_availability_updates_with_live_source_and_query() {
        let config = Config::parse(
            r#"
                [source]
                cmd = "unused"

                [input]
                start_mode = "filter"

                [[filters.choices]]
                key = "w"
                label = "working"
                source = "state"
                value = "working"

                [[filters.choices]]
                key = "i"
                label = "idle"
                source = "$state"
                value = "idle"

                [item]
                template = [["$name"]]
                value = "$name"
            "#,
        )
        .unwrap();
        let item = |state: &str| {
            json!({"name": state, "state": state})
                .as_object()
                .unwrap()
                .clone()
        };
        let mut app = App::new(
            vec![item("working")],
            config.item.clone(),
            config.keybindings.clone(),
            config.filters.clone(),
            config.input.clone(),
            true,
        );
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
        let choice = |symbol: &str, terminal: &Terminal<TestBackend>| {
            (0..60)
                .map(|x| &terminal.backend().buffer()[(x, 0)])
                .find(|cell| cell.symbol() == symbol)
                .unwrap()
                .clone()
        };

        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).fg, color(&config.theme.foreground));
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.border));

        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('w'),
        ));
        app.settle_filter_animation();
        let highlight = color(&config.theme.selection_background);
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).bg, highlight);

        assert!(app.replace_source(vec![item("idle")], 0));
        assert!(app.visible.is_empty());
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).bg, highlight);
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.foreground));

        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('a'),
        ));
        app.settle_filter_animation();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).fg, color(&config.theme.border));
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.foreground));

        assert!(app.replace_source(vec![item("working"), item("idle")], 0));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        for character in "idle".chars() {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(character),
            ));
        }
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        app.settle_filter_animation();
        assert_eq!(app.visible.len(), 1);
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).fg, color(&config.theme.border));
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.foreground));

        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('w'),
        ));
        app.settle_filter_animation();
        assert!(app.visible.is_empty());
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).bg, highlight);
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.foreground));

        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        for _ in 0..4 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Backspace,
            ));
        }
        for character in "zzz".chars() {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(character),
            ));
        }
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::CONTROL,
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('a'),
        ));
        app.settle_filter_animation();
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("w", &terminal).fg, color(&config.theme.border));
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.border));

        assert!(app.replace_source(Vec::new(), 0));
        terminal
            .draw(|frame| render(frame, &mut app, &config))
            .unwrap();
        assert_eq!(choice("a", &terminal).bg, highlight);
        assert_eq!(choice("i", &terminal).fg, color(&config.theme.border));
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
