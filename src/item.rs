use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str, pattern::Pattern};
use serde_json::{Map, Value};

use crate::config::{Alignment, ItemConfig, SegmentConfig, StyledSegment, TokenDefinition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedItem {
    pub rows: Vec<RenderedRow>,
    pub search_text: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedRow {
    pub segments: Vec<RenderedSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSegment {
    pub text: String,
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
    pub align: Alignment,
}

pub fn render_items(
    items: &[Map<String, Value>],
    config: &ItemConfig,
    elapsed_ms: u64,
) -> Vec<RenderedItem> {
    items
        .iter()
        .map(|item| render_item(item, config, elapsed_ms))
        .collect()
}

pub fn render_item(
    item: &Map<String, Value>,
    config: &ItemConfig,
    elapsed_ms: u64,
) -> RenderedItem {
    let mut searchable = Vec::new();
    let rows = config
        .template
        .iter()
        .map(|row| {
            let segments = row
                .iter()
                .map(|segment| {
                    let (rendered, is_searchable) =
                        render_segment(item, &config.tokens, segment, elapsed_ms);
                    if is_searchable && !rendered.text.is_empty() {
                        searchable.push(rendered.text.clone());
                    }
                    rendered
                })
                .collect();
            RenderedRow { segments }
        })
        .collect();

    RenderedItem {
        rows,
        search_text: searchable.join(" "),
        value: resolve(item, &config.value),
    }
}

pub fn matching_indices(items: &[RenderedItem], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let pattern = Pattern::parse(
        query,
        nucleo_matcher::pattern::CaseMatching::Smart,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(MatcherConfig::DEFAULT);
    let mut buffer = Vec::new();
    let mut matches: Vec<_> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            pattern
                .score(Utf32Str::new(&item.search_text, &mut buffer), &mut matcher)
                .map(|score| (index, score))
        })
        .collect();
    matches.sort_unstable_by_key(|&(index, score)| (std::cmp::Reverse(score), index));
    matches.into_iter().map(|(index, _)| index).collect()
}

fn render_segment(
    item: &Map<String, Value>,
    definitions: &[TokenDefinition],
    segment: &SegmentConfig,
    elapsed_ms: u64,
) -> (RenderedSegment, bool) {
    let (token, style, searchable, align) = match segment {
        SegmentConfig::Token(token) => (token.as_str(), None, true, Alignment::Left),
        SegmentConfig::Styled(style) => (
            style.token.as_str(),
            Some(style),
            style.searchable,
            style.align,
        ),
    };
    let name = token.strip_prefix('$').unwrap_or(token);
    let definition = definitions.iter().find(|definition| {
        definition.name == name
            && (definition.when.is_empty()
                || definition
                    .when
                    .iter()
                    .any(|expected| field(item, &definition.source) == *expected))
    });

    let text = definition.map_or_else(
        || {
            if token.starts_with('$') {
                field(item, name)
            } else {
                token.to_owned()
            }
        },
        |definition| definition_text(item, definition, elapsed_ms),
    );
    let (fg, bg, bold) = merged_style(definition, style);
    let fg = fg.map(|value| resolve(item, &value));
    let bg = bg.map(|value| resolve(item, &value));

    (
        RenderedSegment {
            text,
            fg,
            bg,
            bold,
            align,
        },
        searchable,
    )
}

fn definition_text(
    item: &Map<String, Value>,
    definition: &TokenDefinition,
    elapsed_ms: u64,
) -> String {
    if let Some(fps) = definition.animation_fps {
        let frame_ms = 1_000 / u64::from(fps);
        let index = (elapsed_ms / frame_ms) as usize % definition.animation_frames.len();
        return definition.animation_frames[index].clone();
    }
    definition
        .text
        .clone()
        .unwrap_or_else(|| field(item, &definition.source))
}

fn merged_style(
    definition: Option<&TokenDefinition>,
    style: Option<&StyledSegment>,
) -> (Option<String>, Option<String>, bool) {
    let fg = style
        .and_then(|style| style.fg.clone())
        .or_else(|| definition.and_then(|definition| definition.fg.clone()));
    let bg = style
        .and_then(|style| style.bg.clone())
        .or_else(|| definition.and_then(|definition| definition.bg.clone()));
    let bold = style.is_some_and(|style| style.bold)
        || definition.is_some_and(|definition| definition.bold);
    (fg, bg, bold)
}

fn resolve(item: &Map<String, Value>, expression: &str) -> String {
    expression
        .strip_prefix('$')
        .map_or_else(|| expression.to_owned(), |name| field(item, name))
}

fn field(item: &Map<String, Value>, path: &str) -> String {
    let path = path.strip_prefix('$').unwrap_or(path);
    let mut value = path.split('.').next().and_then(|key| item.get(key));
    for key in path.split('.').skip(1) {
        value = value.and_then(|value| value.get(key));
    }
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    fn item_config() -> ItemConfig {
        toml::from_str(
            r#"
            template = [
              ["$name", { token = "$state_icon", searchable = false }, { token = "$meta.pr", align = "right" }],
              ["workspace: ", "$workspace"]
            ]
            value = "$id"

            [[tokens]]
            name = "state_icon"
            source = "state"
            when = ["running"]
            animation_fps = 2
            animation_frames = [".", "o"]
            fg = "green"
            "#,
        )
        .unwrap()
    }

    fn source_item() -> Map<String, Value> {
        json!({
            "id": 42,
            "name": "OpenCode",
            "workspace": "Dotfiles",
            "state": "running",
            "state_color": "green",
            "meta": { "pr": "PR #123" }
        })
        .as_object()
        .unwrap()
        .clone()
    }

    #[test]
    fn expands_multiline_template_styles_and_value() {
        let rendered = render_item(&source_item(), &item_config(), 500);

        assert_eq!(rendered.rows[0].segments[0].text, "OpenCode");
        assert_eq!(rendered.rows[0].segments[1].text, "o");
        assert_eq!(rendered.rows[0].segments[1].fg.as_deref(), Some("green"));
        assert_eq!(rendered.rows[0].segments[2].text, "PR #123");
        assert_eq!(rendered.rows[0].segments[2].align, Alignment::Right);
        assert_eq!(
            rendered.search_text,
            "OpenCode PR #123 workspace:  Dotfiles"
        );
        assert_eq!(rendered.value, "42");
    }

    #[test]
    fn fuzzy_matching_scores_and_filters_items() {
        let mut second = source_item();
        second.insert("name".into(), json!("zzzz"));
        let rendered = render_items(&[second, source_item()], &item_config(), 0);

        assert_eq!(matching_indices(&rendered, "zzzz"), [0]);
        assert_eq!(matching_indices(&rendered, ""), [0, 1]);
    }

    #[test]
    fn resolves_colors_from_source_fields() {
        let mut config = item_config();
        config.tokens[0].fg = Some("$state_color".into());

        let rendered = render_item(&source_item(), &config, 0);

        assert_eq!(rendered.rows[0].segments[1].fg.as_deref(), Some("green"));
    }
}
