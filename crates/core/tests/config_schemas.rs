fn repo_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read_json(path: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(repo_path(path)).unwrap()).unwrap()
}

fn assert_valid(schema_path: &str, instance_path: &str) {
    let schema = read_json(schema_path);
    let instance = read_json(instance_path);
    let validator = jsonschema::validator_for(&schema).expect("schema should compile");
    validator.validate(&instance).unwrap_or_else(|error| {
        panic!("{instance_path} should validate against {schema_path}: {error}")
    });
}

#[test]
fn checked_in_examples_validate_against_json_schemas() {
    assert_valid("docs/schemas/config.schema.json", "config.example.json");
    assert_valid("docs/schemas/secrets.schema.json", "secrets.example.json");
}
