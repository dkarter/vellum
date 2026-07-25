use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[test]
fn meta_001_repository_spec_links_are_complete() {
    let mut scenarios = BTreeMap::new();
    let mut errors = Vec::new();
    for path in files_under(Path::new("openspec/specs"), "md") {
        for (line_number, line) in lines(&path).iter().enumerate() {
            if !line.starts_with("#### Scenario:") {
                continue;
            }
            let Some(id) = line
                .rsplit_once("{#")
                .and_then(|(_, suffix)| suffix.strip_suffix('}'))
            else {
                errors.push(format!("{}:{} scenario has no ID", path.display(), line_number + 1));
                continue;
            };
            if !valid_id(id) {
                errors.push(format!("{}:{} invalid scenario ID {id}", path.display(), line_number + 1));
            } else if let Some(previous) = scenarios.insert(id.to_owned(), path.clone()) {
                errors.push(format!("duplicate scenario {id} in {} and {}", previous.display(), path.display()));
            }
        }
    }

    let mut references: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for root in ["src", "tests"] {
        for path in files_under(Path::new(root), "rs") {
            let contents = lines(&path);
            let mut test_attribute = false;
            for line in contents {
                let trimmed = line.trim();
                if trimmed == "#[test]" {
                    test_attribute = true;
                    continue;
                }
                if !test_attribute || (!trimmed.starts_with("fn ") && !trimmed.starts_with("async fn ")) {
                    continue;
                }
                test_attribute = false;
                let name = trimmed
                    .split_once("fn ")
                    .and_then(|(_, value)| value.split_once('('))
                    .map(|(name, _)| name)
                    .unwrap();
                let ids = ids_from_test_name(name);
                if ids.is_empty() {
                    errors.push(format!("{} test {name} has no scenario ID", path.display()));
                }
                for id in ids {
                    references.entry(id).or_default().push(path.clone());
                }
            }
        }
    }

    let scenario_ids: BTreeSet<_> = scenarios.keys().cloned().collect();
    let reference_ids: BTreeSet<_> = references.keys().cloned().collect();
    for id in scenario_ids.difference(&reference_ids) {
        errors.push(format!("scenario {id} has no test"));
    }
    for id in reference_ids.difference(&scenario_ids) {
        errors.push(format!("test references unknown scenario {id}"));
    }

    assert!(errors.is_empty(), "spec/test link errors:\n{}", errors.join("\n"));
}

fn files_under(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(files_under(&path, extension));
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

fn valid_id(id: &str) -> bool {
    let Some((prefix, number)) = id.rsplit_once('-') else {
        return false;
    };
    prefix.len() >= 2
        && prefix.chars().all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        && number.len() == 3
        && number.chars().all(|character| character.is_ascii_digit())
}

fn ids_from_test_name(name: &str) -> Vec<String> {
    let parts: Vec<_> = name.split('_').collect();
    parts
        .windows(2)
        .filter_map(|parts| {
            let prefix = parts[0];
            let number = parts[1];
            (prefix.len() >= 2
                && prefix.chars().all(|character| character.is_ascii_alphanumeric())
                && number.len() == 3
                && number.chars().all(|character| character.is_ascii_digit()))
            .then(|| format!("{}-{number}", prefix.to_ascii_uppercase()))
        })
        .collect()
}
