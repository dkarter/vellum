use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

#[test]
fn cli_010_process_working_directory_controls_sources_and_actions() {
    let root = TestDir::new();
    fs::write(root.path().join("source-marker"), "present").unwrap();
    fs::create_dir(root.path().join("override")).unwrap();

    let inherited_palette = root.write_palette(
        "inherited.toml",
        r#"
[source]
cmd = "test -f source-marker && printf '[{\"id\":\"only\"}]'"

[item]
template = [["$id"]]
value = "$id"

[actions]
default = "mark"

[[actions.items]]
name = "mark"
label = "Mark"
command = ["sh", "-c", "touch inherited-action-marker"]
"#,
    );
    let output = run_vellum(&inherited_palette, root.path());
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(root.path().join("inherited-action-marker").exists());

    let override_palette = root.write_palette(
        "override.toml",
        r#"
[source]
cmd = "test -f source-marker && printf '[{\"id\":\"only\"}]'"

[item]
template = [["$id"]]
value = "$id"

[actions]
default = "mark"

[[actions.items]]
name = "mark"
label = "Mark"
command = ["sh", "-c", "touch overridden-action-marker"]
cwd = "override"
"#,
    );
    let output = run_vellum(&override_palette, root.path());
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        root.path()
            .join("override/overridden-action-marker")
            .exists()
    );
    assert!(!root.path().join("overridden-action-marker").exists());
}

#[test]
fn cli_011_invalid_process_working_directory_fails_clearly() {
    let root = TestDir::new();
    let palette = root.write_palette(
        "unused.toml",
        "[source]\ncmd = 'exit 99'\n[item]\ntemplate = [['$id']]\nvalue = '$id'\n",
    );
    let missing = root.path().join("missing");
    let file = root.path().join("not-a-directory");
    fs::write(&file, "present").unwrap();

    let output = run_vellum(&palette, &missing);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("failed to resolve working directory"),
        "{error}"
    );
    assert!(error.contains(&missing.display().to_string()), "{error}");

    let output = run_vellum(&palette, &file);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("failed to use working directory"), "{error}");
    assert!(error.contains(&file.display().to_string()), "{error}");
}

fn run_vellum(palette: &Path, cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vellum"))
        .arg(palette)
        .args(["--select-1", "--cwd"])
        .arg(cwd)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("vellum-process-cwd-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write_palette(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
