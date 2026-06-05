// Crate-level lint policy for walls-core.
#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    reason = "walls-core still uses anyhow at public boundaries; detailed error contracts are deferred to the typed-error cleanup."
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
pub mod config;
pub mod ctx;
pub mod library;
pub mod lock;
pub mod paths;
pub mod pipeline;
pub mod quota;
pub mod selection;
pub mod sources;
pub mod state;
pub mod validate;
pub mod wallhaven;

pub use ctx::{RefreshLevel, WallsCtx};
pub use paths::{expand_home, WallsPaths};
