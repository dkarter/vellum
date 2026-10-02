#[cfg(unix)]
#[test]
fn pal_020_reviewed_closed_paginates_resolved_thread_comments() {
    use serde_json::json;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    let root = std::env::temp_dir().join(format!(
        "vellum-gh-fixture-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let exhausted = json!({"hasNextPage":false,"endCursor":null});
    let connection = |nodes| json!({"nodes":nodes,"pageInfo":exhausted});
    let context =
        json!({"data":{"viewer":{"login":"me"},"repository":{"nameWithOwner":"owner/repo"}}});
    let pr = json!({
        "id":"pr1", "number":42, "title":"Reviewed long thread", "url":"https://github.com/owner/repo/pull/42", "state":"MERGED", "author":{"login":"other"},
        "reviews":connection(json!([])), "reviewRequests":connection(json!([])),
        "reviewThreads":connection(json!([{"id":"thread1","isResolved":true,"comments":{
            "nodes":[{"author":{"login":"other"}}],"pageInfo":{"hasNextPage":true,"endCursor":"comments-page-2"}
        }}])),
        "commits":connection(json!([]))
    });
    let search = json!({"data":{"search":connection(json!([pr]))}});
    let comments =
        json!({"data":{"node":{"comments":connection(json!([{"author":{"login":"me"}}]))}}});
    fs::write(root.join("context.json"), context.to_string()).unwrap();
    fs::write(root.join("search.json"), search.to_string()).unwrap();
    fs::write(root.join("comments.json"), comments.to_string()).unwrap();
    let gh = root.join("gh");
    fs::write(
        &gh,
        r#"#!/bin/sh
case "$*" in
  *"repository(owner:"*) cat "$FIXTURE_ROOT/context.json" ;;
  *"search(query:"*) cat "$FIXTURE_ROOT/search.json" ;;
  *"node(id:"*"comments(first:"*) cat "$FIXTURE_ROOT/comments.json" ;;
  *) echo "unexpected fixture request" >&2; exit 1 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o700)).unwrap();
    let palette = root.join("palette.toml");
    fs::write(&palette, "[source]\nbuiltin = 'github-prs'\n[source.remote]\npage_size = 2\n[filters]\ninitial = 'reviewed-closed'\n[[filters.choices]]\nkey = 'v'\nlabel = 'reviewed closed'\nsource = 'category'\nvalue = 'reviewed-closed'\n[frecency]\nenabled = false\n[item]\ntemplate = [['$title']]\nvalue = '$url'").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vlm"))
        .arg(&palette)
        .arg("--select-1")
        .env("PATH", format!("{}:/usr/bin:/bin", root.display()))
        .env("FIXTURE_ROOT", &root)
        .env("VELLUM_CONFIG", root.join("config"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "https://github.com/owner/repo/pull/42\n"
    );
    fs::remove_dir_all(root).unwrap();
}
