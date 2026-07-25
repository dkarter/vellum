use std::str::FromStr;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

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
        let query = if app.query.is_empty() {
            &config.search.placeholder
        } else {
            &app.query
        };
        let input = Paragraph::new(query.as_str())
            .style(base_style(theme))
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_style(Style::new().fg(color(&theme.border)))
                    .title(" Vellum "),
            );
        frame.render_widget(input, search_area);
        if !app.query.is_empty() {
            frame.set_cursor_position((
                search_area.x + 1 + app.query.chars().count() as u16,
                search_area.y + 1,
            ));
        }
    }

    let inner_width = list_area.width.saturating_sub(4) as usize;
    let list_items: Vec<_> = app
        .visible
        .iter()
        .map(|&index| {
            let lines: Vec<_> = app.items[index]
                .rows
                .iter()
                .map(|row| render_row(row, inner_width, theme))
                .collect();
            ListItem::new(lines)
        })
        .collect();
    let list = List::new(list_items)
        .block(
            Block::new()
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(Style::new().fg(color(&theme.border))),
        )
        .highlight_symbol("› ")
        .highlight_style(
            Style::new()
                .fg(color(&theme.selection_foreground))
                .bg(color(&theme.selection_background)),
        );
    let mut state =
        ListState::default().with_selected((!app.visible.is_empty()).then_some(app.selected));
    frame.render_stateful_widget(list, list_area, &mut state);

    let footer = format!(
        "{}/{}  ↑↓ navigate  enter select  esc cancel",
        app.visible.len(),
        app.items.len()
    );
    frame.render_widget(
        Paragraph::new(footer).style(Style::new().fg(color(&theme.border))),
        footer_area,
    );
}

fn render_row<'a>(row: &'a RenderedRow, width: usize, theme: &Theme) -> Line<'a> {
    let split = row
        .segments
        .iter()
        .position(|segment| segment.align == Alignment::Right);
    let (left, right) = split.map_or((&row.segments[..], &[][..]), |index| {
        row.segments.split_at(index)
    });
    let left_width: usize = left
        .iter()
        .map(|segment| segment.text.chars().count())
        .sum();
    let right_width: usize = right
        .iter()
        .map(|segment| segment.text.chars().count())
        .sum();
    let mut spans: Vec<_> = left.iter().map(|segment| span(segment, theme)).collect();
    if !right.is_empty() {
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(left_width + right_width)),
        ));
        spans.extend(right.iter().map(|segment| span(segment, theme)));
    }
    Line::from(spans)
}

fn span<'a>(segment: &'a RenderedSegment, theme: &Theme) -> Span<'a> {
    let mut style = Style::new()
        .fg(segment
            .fg
            .as_deref()
            .map_or_else(|| color(&theme.foreground), color))
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
    fn renders_search_multiline_items_and_footer() {
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
    }
}
