fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let profile_dir = match profile.as_str() {
        "dev" | "debug" => "debug",
        other => other,
    };
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        std::env::var("CARGO_WORKSPACE_DIR")
            .map(|w| format!("{w}/target"))
            .unwrap_or_else(|_| "target".into())
    });
    let tray = format!("{target_dir}/{profile_dir}/walls-tray");
    println!("cargo:rustc-env=WALLS_CLI_TRAY_DEFAULT={tray}");
}
