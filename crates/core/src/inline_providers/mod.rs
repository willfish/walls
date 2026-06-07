//! Inline per-source fetchers (bing/json/mediarss live in `ctx::advance`; these are the rest).

mod apod;
mod attribution;
mod common;
mod immich;
mod pixabay;
mod reddit;
mod spotlight;

pub use apod::try_apod;
pub use attribution::try_attribution;
pub use immich::try_immich;
pub use pixabay::try_pixabay;
pub use reddit::try_reddit;
pub use spotlight::try_spotlight;
