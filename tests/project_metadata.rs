use std::{fs, process::Command};

use serde_json::Value;
use vellum::{config::Config, item::render_item};

#[test]
fn sch_001_bundled_schema_is_valid_json() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string("schemas/vellum.schema.json").expect("schema should be readable"),
    )
    .expect("schema should be valid JSON");

    assert_eq!(schema["title"], "Vellum configuration");
    assert_eq!(schema["$ref"], "./config-options.schema.json");
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

    assert_eq!(schema["$ref"], "./config-options.schema.json");
    assert!(example.starts_with("#:schema ../schemas/global.schema.json"));
}

#[test]
fn sch_004_global_and_palette_schemas_share_option_definitions() {
    let palette: Value =
        serde_json::from_str(&fs::read_to_string("schemas/vellum.schema.json").unwrap()).unwrap();
    let global: Value =
        serde_json::from_str(&fs::read_to_string("schemas/global.schema.json").unwrap()).unwrap();
    let shared: Value =
        serde_json::from_str(&fs::read_to_string("schemas/config-options.schema.json").unwrap())
            .unwrap();

    assert_eq!(palette["$ref"], "./config-options.schema.json");
    assert_eq!(global["$ref"], palette["$ref"]);
    assert!(shared["properties"]["search"]["properties"]["title"].is_object());
    assert!(shared["properties"]["theme"]["properties"]["insert_mode_background"].is_object());
}

#[test]
fn sch_005_shared_schema_describes_palette_filters() {
    let shared: Value =
        serde_json::from_str(&fs::read_to_string("schemas/config-options.schema.json").unwrap())
            .unwrap();

    assert_eq!(
        shared["properties"]["filters"]["properties"]["label"]["default"],
        "filter"
    );
    assert_eq!(
        shared["properties"]["filters"]["properties"]["mode"]["default"],
        "ctrl-g"
    );
    assert_eq!(
        shared["properties"]["filters"]["properties"]["clear"]["default"],
        "a"
    );
    assert_eq!(
        shared["$defs"]["filter-choice"]["required"],
        serde_json::json!(["key", "label", "source", "value"])
    );
    assert_eq!(
        shared["$defs"]["filter-choice"]["properties"]["key"]["$ref"],
        "#/$defs/filter-choice-bindings"
    );
    assert_eq!(
        shared["$defs"]["filter-choice-bindings"]["oneOf"][1]["minItems"],
        1
    );
    assert!(shared["$defs"]["filter-choice"]["properties"]["icon"].is_object());
    assert!(shared["$defs"]["filter-choice"]["properties"]["fg"].is_object());
}

#[test]
fn sch_006_shared_schema_describes_native_actions() {
    let shared: Value =
        serde_json::from_str(&fs::read_to_string("schemas/config-options.schema.json").unwrap())
            .unwrap();

    let actions = &shared["properties"]["actions"]["properties"];
    assert_eq!(actions["menu"]["default"], "ctrl-a");
    assert!(actions["default"].is_object());
    assert_eq!(actions["items"]["items"]["$ref"], "#/$defs/action");
    let action = &shared["$defs"]["action"];
    assert!(action["properties"]["command"].is_object());
    assert!(action["properties"]["shell"].is_object());
    assert!(action["properties"]["cwd"].is_object());
    assert_eq!(
        action["properties"]["availability"]["$ref"],
        "#/$defs/action-availability"
    );
    assert_eq!(
        shared["$defs"]["action-availability"]["properties"]["cache_ms"]["default"],
        30_000
    );
    assert_eq!(
        shared["$defs"]["action-availability"]["properties"]["timeout_ms"]["default"],
        5_000
    );
    assert_eq!(action["properties"]["icon"]["default"], "");
    assert_eq!(action["properties"]["description"]["default"], "");
    assert_eq!(
        action["properties"]["when"]["items"]["$ref"],
        "#/$defs/action-condition"
    );
    assert!(shared["$defs"]["action-condition"]["oneOf"].is_array());
    assert_eq!(
        action["properties"]["on_success"]["enum"],
        serde_json::json!(["exit", "refresh"])
    );
    assert!(action["oneOf"].is_array());
}

#[test]
fn itm_003_icon_agent_example_rewrites_known_agents_and_preserves_unknown_agents() {
    let example = fs::read_to_string("examples/herdr-agents-icons.toml").unwrap();
    let config = Config::parse(&example).unwrap();
    let known = serde_json::json!({ "pane_id": "one", "agent": "opencode" });
    let unknown = serde_json::json!({ "pane_id": "two", "agent": "future-agent" });

    let known = render_item(known.as_object().unwrap(), &config.item, 0);
    let unknown = render_item(unknown.as_object().unwrap(), &config.item, 0);

    assert_eq!(known.rows[0].segments[2].text, " OpenCode");
    assert_eq!(unknown.rows[0].segments[2].text, "future-agent");
    assert_eq!(config.item.tokens.last().unwrap().source, "agent");
    assert!(config.item.tokens.last().unwrap().when.is_empty());
}
