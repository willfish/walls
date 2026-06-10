fn repo_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read_json(path: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(repo_path(path)).unwrap()).unwrap()
}

fn read_text(path: &str) -> String {
    std::fs::read_to_string(repo_path(path)).unwrap()
}

fn assert_valid(schema_path: &str, instance_path: &str) {
    let schema = read_json(schema_path);
    let instance = read_json(instance_path);
    let validator = jsonschema::validator_for(&schema).expect("schema should compile");
    validator.validate(&instance).unwrap_or_else(|error| {
        panic!("{instance_path} should validate against {schema_path}: {error}")
    });
}

fn collect_schema_field_paths(schema: &serde_json::Value, prefix: &str) -> Vec<String> {
    let mut paths = Vec::new();
    collect_schema_node(schema, schema, prefix, &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn collect_schema_node(
    root: &serde_json::Value,
    node: &serde_json::Value,
    prefix: &str,
    paths: &mut Vec<String>,
) {
    if let Some(reference) = node.get("$ref").and_then(|value| value.as_str()) {
        let pointer = reference
            .strip_prefix('#')
            .expect("local schema reference should start with #");
        let resolved = root
            .pointer(pointer)
            .unwrap_or_else(|| panic!("schema reference {reference} should resolve"));
        collect_schema_node(root, resolved, prefix, paths);
        return;
    }

    if let Some(properties) = node.get("properties").and_then(|value| value.as_object()) {
        for (name, property) in properties {
            let child_prefix = format!("{prefix}.{name}");
            collect_schema_node(root, property, &child_prefix, paths);
        }
        return;
    }

    if let Some(items) = node.get("items") {
        if items.get("$ref").is_some() || items.get("properties").is_some() {
            collect_schema_node(root, items, &format!("{prefix}[]"), paths);
        } else {
            paths.push(prefix.to_string());
        }
        return;
    }

    if let Some(additional_properties) = node.get("additionalProperties") {
        if additional_properties.is_object() {
            collect_schema_node(root, additional_properties, &format!("{prefix}.*"), paths);
            return;
        }
    }

    paths.push(prefix.to_string());
}

#[test]
fn checked_in_examples_validate_against_json_schemas() {
    assert_valid("docs/schemas/config.schema.json", "config.example.json");
    assert_valid("docs/schemas/secrets.schema.json", "secrets.example.json");
}

#[test]
fn tui_config_coverage_matrix_classifies_persisted_schema_fields() {
    let docs = read_text("docs/tui.md");
    assert!(
        docs.contains("### Config tab field coverage matrix"),
        "docs/tui.md should document Config tab field coverage"
    );

    for coverage in ["Editable", "Read-only", "Manual"] {
        assert!(
            docs.contains(&format!("| {coverage} |")),
            "coverage matrix should include {coverage} rows"
        );
    }

    let mut schema_paths =
        collect_schema_field_paths(&read_json("docs/schemas/config.schema.json"), "config");
    schema_paths.extend(collect_schema_field_paths(
        &read_json("docs/schemas/secrets.schema.json"),
        "secrets",
    ));

    for path in schema_paths {
        assert!(
            docs.contains(&format!("`{path}`")),
            "Config tab coverage matrix should classify `{path}`"
        );
    }
}
