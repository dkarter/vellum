use std::{fs, process::Command};

#[test]
fn cli_011_shell_completion_scripts_and_live_palette_candidates() {
    let root = std::env::temp_dir().join(format!(
        "vellum-completions-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let palettes = root.join("vellum/palettes");
    fs::create_dir_all(&palettes).unwrap();
    fs::write(palettes.join("my-palette.toml"), "invalid palette").unwrap();
    fs::write(palettes.join("other.json"), "{}").unwrap();
    fs::create_dir(palettes.join("directory.toml")).unwrap();

    let binary = env!("CARGO_BIN_EXE_vlm");
    for shell in ["bash", "zsh", "fish", "elvish", "nu", "powershell"] {
        let output = Command::new(binary)
            .arg(format!("--completions={shell}"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("__complete_word__"));
    }

    let complete = |line: &str, word: &str| {
        let output = Command::new(binary)
            .env("XDG_CONFIG_HOME", &root)
            .arg("__complete_word__")
            .args([
                "--shell",
                "bash",
                "--line",
                line,
                "--bash-word",
                word,
                "--bash-wordbreaks",
                " ",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let root_candidates = complete("vlm ", "");
    assert!(
        root_candidates.lines().any(|line| line == "my-palette"),
        "{root_candidates}"
    );
    assert!(
        root_candidates.lines().any(|line| line == "palettes"),
        "{root_candidates}"
    );
    assert!(!root_candidates.contains("other.json"));
    assert!(!root_candidates.contains("directory.toml"));
    assert!(complete("vlm --preview-position ", "").contains("bottom"));
    assert!(complete("vlm palettes ", "").contains("sync"));
    assert!(complete("vlm palettes sync --", "--").contains("--overwrite"));

    fs::remove_dir_all(root).unwrap();
}
