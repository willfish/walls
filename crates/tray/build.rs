fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        std::env::var("CARGO_WORKSPACE_DIR")
            .map(|w| format!("{w}/target"))
            .unwrap_or_else(|_| "target".into())
    });
    let walls = format!("{target_dir}/{profile}/walls");
    println!("cargo:rustc-env=WALLS_TRAY_WALLS_DEFAULT={walls}");
}
