use std::path::Path;

use serde_json::json;

include!("files.rs");

pub fn write_minimal_config(root: &Path, image_dir: &Path, noop: &Path) {
    let config = json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": paths_block(root),
        "apply": apply_block(noop),
        "display": { "mode": "os" },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "folder", "path": image_dir.display().to_string() }
        ],
    });
    write_config(root, config);
    write_secrets(root, json!({}));
}
