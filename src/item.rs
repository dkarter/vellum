use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str, pattern::Pattern};
use serde_json::{Map, Value};

use crate::{
    config::{Alignment, ItemConfig, RepeatedSegment, SegmentConfig, TokenDefinition},
    frecency::FrecencyRank,
};

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
            let mut segments = Vec::new();
            for segment in row {
                if let SegmentConfig::Repeated(repeated) = segment {
                    for (rendered, is_searchable) in
                        render_repeated_segments(item, &config.tokens, repeated, elapsed_ms)
                    {
                        if is_searchable {
                            searchable.push(rendered.text.clone());
                        }
                        segments.push(rendered);
                    }
                    continue;
                }
                let (rendered, is_searchable) =
                    render_segment(item, &config.tokens, segment, elapsed_ms);
                if is_searchable && !rendered.text.is_empty() {
                    searchable.push(rendered.text.clone());
                }
                segments.push(rendered);
            }
            RenderedRow { segments }
        })
        .collect();

    RenderedItem {
        rows,
        search_text: searchable.join(" "),
        value: resolve(item, &config.value),
    }
}

fn render_repeated_segments(
    item: &Map<String, Value>,
    definitions: &[TokenDefinition],
    repeated: &RepeatedSegment,
    elapsed_ms: u64,
) -> Vec<(RenderedSegment, bool)> {
    let Some(values) = field_value(item, &repeated.for_each).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut segments = Vec::with_capacity(values.len());
    let mut seen = repeated.unique.then(HashSet::new);
    for value in values {
        let context = value.as_object().map_or_else(
            || RenderContext::scalar(value, item),
            |element| RenderContext::element(element, item),
        );
        let presentation = SegmentPresentation {
            token: &repeated.token,
            fg: repeated.fg.as_deref(),
            bg: repeated.bg.as_deref(),
            bold: repeated.bold,
            searchable: repeated.searchable,
            align: repeated.align,
        };
        let (mut rendered, searchable) =
            render_token(context, definitions, presentation, elapsed_ms);
        if rendered.text.is_empty() {
            continue;
        }
        if seen
            .as_mut()
            .is_some_and(|seen| !seen.insert(rendered.text.clone()))
        {
            continue;
        }
        if !segments.is_empty() {
            rendered.text.insert_str(0, &repeated.separator);
        }
        segments.push((rendered, searchable));
    }
    segments
}

pub fn matching_indices(items: &[RenderedItem], query: &str) -> Vec<usize> {
    matching_indices_with_frecency(items, query, &HashMap::new())
}

pub fn matching_indices_with_frecency(
    items: &[RenderedItem],
    query: &str,
    frecency: &HashMap<String, FrecencyRank>,
) -> Vec<usize> {
    matching_indices_with_frecency_by(items, query, frecency, |_| true)
}

pub fn matching_indices_with_frecency_by(
    items: &[RenderedItem],
    query: &str,
    frecency: &HashMap<String, FrecencyRank>,
    mut include: impl FnMut(usize) -> bool,
) -> Vec<usize> {
    let mut matches: Vec<_> = if query.is_empty() {
        (0..items.len())
            .filter(|&index| include(index))
            .map(|index| (index, 0))
            .collect()
    } else {
        let pattern = Pattern::parse(
            query,
            nucleo_matcher::pattern::CaseMatching::Smart,
            nucleo_matcher::pattern::Normalization::Smart,
        );
        let mut matcher = Matcher::new(MatcherConfig::DEFAULT);
        let mut buffer = Vec::new();
        items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if !include(index) {
                    return None;
                }
                pattern
                    .score(Utf32Str::new(&item.search_text, &mut buffer), &mut matcher)
                    .map(|score| (index, score))
            })
            .collect()
    };
    if query.is_empty() && frecency.is_empty() {
        return matches.into_iter().map(|(index, _)| index).collect();
    }
    matches.sort_unstable_by(|&(left_index, left_match), &(right_index, right_match)| {
        let left = frecency.get(&items[left_index].value);
        let right = frecency.get(&items[right_index].value);
        match (left, right) {
            (Some(left), Some(right)) => right.cmp(left),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| right_match.cmp(&left_match))
        .then_with(|| left_index.cmp(&right_index))
    });
    matches.into_iter().map(|(index, _)| index).collect()
}

fn render_segment(
    item: &Map<String, Value>,
    definitions: &[TokenDefinition],
    segment: &SegmentConfig,
    elapsed_ms: u64,
) -> (RenderedSegment, bool) {
    let presentation = match segment {
        SegmentConfig::Token(token) => SegmentPresentation {
            token,
            fg: None,
            bg: None,
            bold: false,
            searchable: true,
            align: Alignment::Left,
        },
        SegmentConfig::Styled(style) => SegmentPresentation {
            token: &style.token,
            fg: style.fg.as_deref(),
            bg: style.bg.as_deref(),
            bold: style.bold,
            searchable: style.searchable,
            align: style.align,
        },
        SegmentConfig::Repeated(_) => {
            unreachable!("repeated segments are expanded before rendering")
        }
    };
    render_token(
        RenderContext::root(item),
        definitions,
        presentation,
        elapsed_ms,
    )
}

#[derive(Clone, Copy)]
struct SegmentPresentation<'a> {
    token: &'a str,
    fg: Option<&'a str>,
    bg: Option<&'a str>,
    bold: bool,
    searchable: bool,
    align: Alignment,
}

#[derive(Clone, Copy)]
struct RenderContext<'a> {
    element: Option<&'a Map<String, Value>>,
    parent: Option<&'a Map<String, Value>>,
    scalar: Option<&'a Value>,
}

impl<'a> RenderContext<'a> {
    fn root(item: &'a Map<String, Value>) -> Self {
        Self {
            element: Some(item),
            parent: None,
            scalar: None,
        }
    }

    fn element(element: &'a Map<String, Value>, parent: &'a Map<String, Value>) -> Self {
        Self {
            element: Some(element),
            parent: Some(parent),
            scalar: None,
        }
    }

    fn scalar(value: &'a Value, parent: &'a Map<String, Value>) -> Self {
        Self {
            element: None,
            parent: Some(parent),
            scalar: Some(value),
        }
    }

    fn value(self, path: &str) -> Option<&'a Value> {
        let path = path.strip_prefix('$').unwrap_or(path);
        if path == "value" {
            return self
                .scalar
                .or_else(|| self.element.and_then(|element| field_value(element, path)));
        }
        if let Some(path) = path.strip_prefix("parent.") {
            return self.parent.and_then(|parent| field_value(parent, path));
        }
        self.element.and_then(|element| field_value(element, path))
    }

    fn text(self, path: &str) -> String {
        value_text(self.value(path))
    }

    fn matches(self, path: &str, expected: &str) -> bool {
        match self.value(path) {
            Some(Value::String(value)) => value == expected,
            Some(Value::Bool(value)) => expected.parse() == Ok(*value),
            Some(Value::Number(value)) => expected
                .parse::<serde_json::Number>()
                .is_ok_and(|expected| &expected == value),
            Some(Value::Null) => expected == "null",
            Some(Value::Array(_) | Value::Object(_)) => false,
            None => false,
        }
    }

    fn resolve(self, expression: &str) -> String {
        expression
            .strip_prefix('$')
            .map_or_else(|| expression.to_owned(), |path| self.text(path))
    }
}

fn render_token(
    context: RenderContext<'_>,
    definitions: &[TokenDefinition],
    presentation: SegmentPresentation<'_>,
    elapsed_ms: u64,
) -> (RenderedSegment, bool) {
    let name = presentation
        .token
        .strip_prefix('$')
        .unwrap_or(presentation.token);
    let definition = definitions.iter().find(|definition| {
        definition.name == name
            && (definition.when.is_empty()
                || definition
                    .when
                    .iter()
                    .any(|expected| context.matches(&definition.source, expected)))
    });

    let text = definition.map_or_else(
        || {
            if presentation.token.starts_with('$') {
                context.text(name)
            } else {
                presentation.token.to_owned()
            }
        },
        |definition| definition_text(context, definition, elapsed_ms),
    );
    let fg = presentation
        .fg
        .map(str::to_owned)
        .or_else(|| definition.and_then(|definition| definition.fg.clone()))
        .map(|value| context.resolve(&value));
    let bg = presentation
        .bg
        .map(str::to_owned)
        .or_else(|| definition.and_then(|definition| definition.bg.clone()))
        .map(|value| context.resolve(&value));
    let bold = presentation.bold || definition.is_some_and(|definition| definition.bold);

    (
        RenderedSegment {
            text,
            fg,
            bg,
            bold,
            align: presentation.align,
        },
        presentation.searchable,
    )
}

fn definition_text(
    context: RenderContext<'_>,
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
        .unwrap_or_else(|| context.text(&definition.source))
}

fn resolve(item: &Map<String, Value>, expression: &str) -> String {
    expression
        .strip_prefix('$')
        .map_or_else(|| expression.to_owned(), |name| field(item, name))
}

pub(crate) fn field_value<'a>(item: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let path = path.strip_prefix('$').unwrap_or(path);
    let mut parts = path.split('.');
    let mut value = parts.next().and_then(|key| item.get(key));
    for key in parts {
        value = value.and_then(|value| value.get(key));
    }
    value
}

pub(crate) fn field_value_at<'a>(item: &'a Map<String, Value>, path: &[&str]) -> Option<&'a Value> {
    let mut parts = path.iter();
    let mut value = parts.next().and_then(|key| item.get(*key));
    for key in parts {
        value = value.and_then(|value| value.get(*key));
    }
    value
}

fn field(item: &Map<String, Value>, path: &str) -> String {
    value_text(field_value(item, path))
}

fn value_text(value: Option<&Value>) -> String {
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
    fn itm_001_expands_multiline_template_styles_and_value() {
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
    fn sea_001_fuzzy_matching_scores_and_filters_items() {
        let mut second = source_item();
        second.insert("name".into(), json!("zzzz"));
        let rendered = render_items(&[second, source_item()], &item_config(), 0);

        assert_eq!(matching_indices(&rendered, "zzzz"), [0]);
        assert_eq!(matching_indices(&rendered, ""), [0, 1]);
    }

    #[test]
    fn itm_002_resolves_colors_from_source_fields() {
        let mut config = item_config();
        config.tokens[0].fg = Some("$state_color".into());

        let rendered = render_item(&source_item(), &config, 0);

        assert_eq!(rendered.rows[0].segments[1].fg.as_deref(), Some("green"));
    }

    #[test]
    fn itm_004_renders_mapped_array_elements_as_aligned_segments() {
        let config: ItemConfig = toml::from_str(
            r#"
            template = [
              [{ for_each = "$agents", token = "$agent_icon", separator = " ", unique = true, fg = "$parent.color", bold = true, searchable = false, align = "right" }],
              [{ for_each = "$labels", token = "$label", separator = ", " }],
              [{ for_each = "$optional", token = "$name", separator = ", ", searchable = false }],
              [{ for_each = "$missing", token = "$name" }],
              [{ for_each = "$not_array", token = "$name" }]
            ]
            value = "$id"

            [[tokens]]
            name = "agent_icon"
            source = "agent"
            when = ["opencode"]
            text = "OC"

            [[tokens]]
            name = "agent_icon"
            source = "agent"
            text = "?"

            [[tokens]]
            name = "label"
            source = "value"
            "#,
        )
        .unwrap();
        let item = json!({
            "id": "workspace",
            "color": "cyan",
            "agents": [{ "agent": "opencode" }, { "agent": "opencode" }, { "agent": "future" }],
            "labels": ["one", "two"],
            "optional": [{}, { "name": "shown" }, {}],
            "not_array": "ignored"
        });

        let rendered = render_item(item.as_object().unwrap(), &config, 0);

        assert_eq!(rendered.rows[0].segments[0].text, "OC");
        assert_eq!(rendered.rows[0].segments[1].text, " ?");
        assert!(
            rendered.rows[0]
                .segments
                .iter()
                .all(|segment| segment.align == Alignment::Right)
        );
        assert!(
            rendered.rows[0]
                .segments
                .iter()
                .all(|segment| segment.fg.as_deref() == Some("cyan") && segment.bold)
        );
        assert_eq!(rendered.rows[1].segments[0].text, "one");
        assert_eq!(rendered.rows[1].segments[1].text, ", two");
        assert_eq!(rendered.rows[2].segments[0].text, "shown");
        assert!(rendered.rows[3].segments.is_empty());
        assert!(rendered.rows[4].segments.is_empty());
        assert_eq!(rendered.search_text, "one , two");
    }

    #[test]
    fn frc_004_selected_entries_rank_by_frecency_and_exact_recency() {
        let items = [
            RenderedItem {
                rows: Vec::new(),
                search_text: "agent one".into(),
                value: "one".into(),
            },
            RenderedItem {
                rows: Vec::new(),
                search_text: "agent two".into(),
                value: "two".into(),
            },
            RenderedItem {
                rows: Vec::new(),
                search_text: "agent three".into(),
                value: "three".into(),
            },
        ];
        let scores = HashMap::from([
            (
                "one".into(),
                FrecencyRank {
                    score: 100,
                    last_access: 10,
                },
            ),
            (
                "two".into(),
                FrecencyRank {
                    score: 100,
                    last_access: 20,
                },
            ),
        ]);

        assert_eq!(
            matching_indices_with_frecency(&items, "agent", &scores),
            [1, 0, 2]
        );
        assert_eq!(
            matching_indices_with_frecency(&items, "", &scores),
            [1, 0, 2]
        );
    }
}
