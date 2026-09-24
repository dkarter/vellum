//! Completion metadata and the fast, isolated completion request path.

use std::{fs, path::Path};

use usage::{
    Cli,
    complete::{Candidate, CompleteCtx, Shell},
};

/// The completion grammar mirrors the public CLI without parsing a palette.
#[allow(dead_code)] // The derive consumes this declaration for completion metadata.
#[derive(Cli)]
#[usage(bin = "vlm", version, completion)]
struct CompletionCli {
    #[usage(flatten)]
    run: RunCompletion,
    #[usage(subcommand)]
    command: Option<Commands>,
}

#[derive(usage::Args)]
struct RunCompletion {
    /// Palette name or TOML path
    #[usage(arg, name = "PALETTE", complete = palette_candidates)]
    palette: Option<String>,
    /// Auto-detect lines, JSON or NDJSON from stdin
    #[usage(long)]
    stdin: bool,
    /// Wrap input lines in a field
    #[usage(long, value_name = "FIELD")]
    lines: Option<String>,
    /// Transform JSON input with jq
    #[usage(long, value_name = "FILTER")]
    jq: Option<String>,
    /// Map a source field to a target field
    #[usage(long, value_name = "TARGET=SOURCE")]
    field: Vec<String>,
    /// Accept the sole initial result
    #[usage(short = '1', long)]
    select_1: bool,
    /// Enable preview
    #[usage(long)]
    preview: bool,
    /// Hide preview
    #[usage(long)]
    no_preview: bool,
    /// Set preview position
    #[usage(long, choices("left", "right", "top", "bottom"), value_name = "PLACE")]
    preview_position: Option<String>,
    /// Print a shell completion script
    #[usage(long, choices("bash", "zsh", "fish", "elvish", "nu", "powershell"))]
    completions: Option<String>,
}

#[derive(usage::Subcommands)]
enum Commands {
    /// Manage palettes
    Palettes(Palettes),
}

#[derive(usage::Args)]
struct Palettes {
    #[usage(subcommand)]
    command: SyncCommand,
}

#[derive(usage::Subcommands)]
enum SyncCommand {
    /// Install bundled palettes
    Sync(Sync),
}

#[derive(usage::Args)]
struct Sync {
    /// Replace existing palettes
    #[usage(long)]
    overwrite: bool,
}

fn palette_candidates(
    _partial: &<RunCompletion as usage::spec::CommandArgs>::Partial,
    ctx: &CompleteCtx<'_>,
) -> Vec<Candidate<'static>> {
    palette_names(super::config_root().as_deref(), ctx.prefix)
        .into_iter()
        .map(Candidate::new)
        .collect()
}

fn palette_names(config_root: Option<&Path>, prefix: &str) -> Vec<String> {
    let Some(root) = config_root else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(root.join("palettes")) else {
        return Vec::new();
    };
    let mut names: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let stem = name.to_str()?.strip_suffix(".toml")?;
            (stem.starts_with(prefix) && entry.path().is_file()).then(|| stem.to_owned())
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

pub fn request(args: &[std::ffi::OsString]) -> Option<String> {
    CompletionCli::completion_request(args)
}

pub fn script(shell: &str) -> Option<String> {
    Shell::from_name(shell).map(CompletionCli::completion_script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_011_completion_lists_palette_names_without_loading_files() {
        let dir = std::env::temp_dir().join(format!("vellum-completion-{}", std::process::id()));
        let palettes = dir.join("palettes");
        fs::create_dir_all(&palettes).unwrap();
        fs::write(palettes.join("alpha.toml"), "not valid TOML").unwrap();
        fs::write(palettes.join("beta.json"), "").unwrap();
        fs::create_dir_all(palettes.join("nested.toml")).unwrap();
        assert_eq!(palette_names(Some(&dir), ""), ["alpha"]);
        assert_eq!(palette_names(Some(&dir), "al"), ["alpha"]);
        assert!(palette_names(Some(&dir), "be").is_empty());
        fs::remove_dir_all(dir).unwrap();
        assert!(palette_names(None, "").is_empty());
    }
}
