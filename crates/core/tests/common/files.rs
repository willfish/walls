pub fn write_noop_script(root: &std::path::Path) -> std::path::PathBuf {
    let noop = root.join("noop.sh");
    std::fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&noop, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    noop
}

pub fn write_config(root: &std::path::Path, config: serde_json::Value) {
    std::fs::write(
        root.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
}

pub fn write_secrets(root: &std::path::Path, secrets: serde_json::Value) {
    std::fs::write(
        root.join("secrets.json"),
        serde_json::to_string_pretty(&secrets).unwrap(),
    )
    .unwrap();
}

pub fn paths_block(root: &std::path::Path) -> serde_json::Value {
    serde_json::json!({
        "cache_dir": root.join("cache").display().to_string(),
        "download_dir": root.join("downloaded").display().to_string(),
        "favorites_dir": root.join("favorites").display().to_string(),
        "fetched_dir": root.join("fetched").display().to_string(),
        "compose_dir": root.join("wallpaper").display().to_string(),
    })
}

pub fn apply_block(noop: &std::path::Path) -> serde_json::Value {
    serde_json::json!({
        "backend": "custom-script",
        "custom_script": noop.display().to_string(),
    })
}
