mod common {
    include!("common/minimal.rs");
}

use std::fs;
use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;
use walls_core::events::{read_events, EventKind};
use walls_core::WallsCtx;

#[test]
fn current_path_returns_composed_file() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let wall = images.join("wall.jpg");
    fs::write(&wall, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    assert!(ctx.current_path().is_none());

    ctx.apply_file(&wall, ApplyTrigger::Manual).unwrap();
    assert_eq!(ctx.current_path(), Some(PathBuf::from(&wall).as_path()));
    assert!(ctx.current_meta().is_some());
    let events = read_events(&ctx.paths.event_journal_file).unwrap();
    assert!(matches!(
        events.as_slice(),
        [walls_core::events::EventRecord {
            kind: EventKind::Apply {
                trigger: ApplyTrigger::Manual,
                ..
            },
            ..
        }]
    ));
}
