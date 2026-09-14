use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temp_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "vellum-{label}-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn run_palette(palette: &Path, cwd: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vellum"));
    command.arg(palette).arg("--select-1");
    if let Some(cwd) = cwd {
        command.env("VELLUM_TEST_CWD", cwd);
    } else {
        command.env_remove("VELLUM_TEST_CWD");
    }
    command.output().unwrap()
}

#[test]
fn cfg_012_sources_and_actions_inherit_the_palette_working_directory() {
    let root = temp_directory("configured-cwd");
    fs::write(root.join("source-marker"), "present").unwrap();
    let palette = root.with_extension("toml");
    fs::write(
        &palette,
        r#"
            cwd = "$VELLUM_TEST_CWD"

            [source]
            cmd = "test -f source-marker && printf '[{\"id\":\"only\"}]'"

            [actions]
            default = "record"

            [[actions.items]]
            name = "record"
            label = "Record directory"
            command = ["sh", "-c", "pwd > action-directory"]

            [item]
            value = "$id"
            template = [["$id"]]
        "#,
    )
    .unwrap();

    let output = run_palette(&palette, Some(&root));

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("action-directory"))
            .unwrap()
            .trim(),
        root.canonicalize().unwrap().to_string_lossy()
    );
    fs::remove_file(palette).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cfg_013_action_working_directories_override_the_palette_directory() {
    let root = temp_directory("action-cwd-override");
    let action_directory = root.join("action");
    fs::create_dir(&action_directory).unwrap();
    let palette = root.with_extension("toml");
    fs::write(
        &palette,
        r#"
            cwd = "$VELLUM_TEST_CWD"

            [source]
            cmd = "printf '[{\"id\":\"only\"}]'"

            [actions]
            default = "record"

            [[actions.items]]
            name = "record"
            label = "Record directory"
            command = ["sh", "-c", "touch action-marker"]
            cwd = "action"

            [item]
            value = "$id"
            template = [["$id"]]
        "#,
    )
    .unwrap();

    let output = run_palette(&palette, Some(&root));

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(action_directory.join("action-marker").exists());
    assert!(!root.join("action-marker").exists());
    fs::remove_file(palette).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cfg_014_invalid_palette_working_directories_fail_clearly() {
    let root = temp_directory("invalid-cwd");
    let palette = root.join("palette.toml");
    fs::write(
        &palette,
        r#"
            cwd = "$VELLUM_TEST_CWD"
            [source]
            cmd = "printf '[{\"id\":\"only\"}]'"
            [item]
            value = "$id"
            template = [["$id"]]
        "#,
    )
    .unwrap();

    let unset = run_palette(&palette, None);
    let missing = run_palette(&palette, Some(&root.join("missing")));

    assert!(!unset.status.success());
    assert!(
        String::from_utf8_lossy(&unset.stderr).contains("VELLUM_TEST_CWD"),
        "{}",
        String::from_utf8_lossy(&unset.stderr)
    );
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("configured cwd"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let empty = fs::read_to_string(&palette)
        .unwrap()
        .replace("$VELLUM_TEST_CWD", "");
    fs::write(&palette, empty).unwrap();
    let empty = run_palette(&palette, None);
    assert!(!empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stderr).contains("cwd cannot be empty"));
    fs::remove_dir_all(root).unwrap();
}
