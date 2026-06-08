// Crate-level lint policy for walls-core.
#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    reason = "some lower-level public helpers still return anyhow; the main context API now exposes typed errors."
)]
#![allow(
    clippy::missing_panics_doc,
    reason = "remaining panics are invariant checks such as static regex construction, not caller-facing contracts."
)]
#![allow(
    clippy::must_use_candidate,
    reason = "walls-core exposes constructors, parsers, and query helpers where enforcing must_use everywhere adds noise before the API split."
)]

pub mod apply;
pub mod autostart;
pub mod bin_resolve;
pub mod config;
pub mod cosmic_theme;
pub mod ctx;
pub mod downloads;
pub mod error;
pub mod feeds;
pub mod inline_providers;
pub mod library;
pub mod lock;
pub mod paths;
pub mod pipeline;
pub mod providers;
pub mod quota;
pub mod rotation;
pub mod selection;
pub mod sources;
pub mod state;
pub mod tray;
pub mod tray_icon;
pub mod unsplash;
pub mod validate;
pub mod wallhaven;

pub use ctx::{RefreshLevel, WallsCtx};
pub use error::{Result, WallsError};
pub use paths::{expand_home, WallsPaths};
