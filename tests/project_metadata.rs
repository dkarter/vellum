use std::{fs, process::Command};

use serde_json::Value;

#[test]
fn sch_001_bundled_schema_is_valid_json() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string("schemas/vellum.schema.json").expect("schema should be readable"),
    )
    .expect("schema should be valid JSON");

    assert_eq!(schema["title"], "Vellum configuration");
    assert!(schema["properties"]["item"]["properties"]["padding"].is_object());
}

#[test]
fn sch_002_taplo_rule_references_bundled_schema() {
    let config: toml::Value =
        toml::from_str(&fs::read_to_string("taplo.toml").expect("Taplo config should be readable"))
            .expect("Taplo config should be valid TOML");

    assert_eq!(
        config["rule"][0]["schema"]["path"].as_str(),
        Some("schemas/vellum.schema.json")
    );
}

#[test]
fn meta_002_spec_runner_resolves_all_file_scenarios() {
    let output = Command::new("bash")
        .args(["scripts/test-spec.sh", "--list", "source-ingestion"])
        .output()
        .expect("spec runner should execute");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "src_001\nsrc_002\nsrc_003\nsrc_004\nsrc_005\n"
    );
}

#[test]
fn sch_003_global_configuration_has_a_dedicated_schema() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string("schemas/global.schema.json")
            .expect("global schema should be readable"),
    )
    .expect("global schema should be valid JSON");
    let example = fs::read_to_string("examples/global.toml").unwrap();

    assert_eq!(schema["$ref"], "./vellum.schema.json");
    assert!(example.starts_with("#:schema ../schemas/global.schema.json"));
}
