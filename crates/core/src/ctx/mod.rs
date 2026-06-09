use crate::config::{Config, Secrets};
use crate::paths::WallsPaths;
use crate::providers::ProviderStatusReport;
use crate::state::State;
use std::str::FromStr;

mod advance;
mod apply;
mod load;
mod state_ops;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLevel {
    All,
    FiltersAndTexts,
    Texts,
    ClockOnly,
}

impl RefreshLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::FiltersAndTexts => "filters-and-texts",
            Self::Texts => "texts",
            Self::ClockOnly => "clock-only",
        }
    }

    pub(super) fn recomposes_image(self) -> bool {
        matches!(self, Self::All | Self::FiltersAndTexts)
    }
}

impl FromStr for RefreshLevel {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(Self::All),
            "filters-and-texts" | "filters_and_texts" => Ok(Self::FiltersAndTexts),
            "texts" => Ok(Self::Texts),
            "clock-only" | "clock_only" => Ok(Self::ClockOnly),
            _ => anyhow::bail!(
                "unsupported refresh level '{value}' (expected all, filters-and-texts, texts, or clock-only)"
            ),
        }
    }
}

pub struct WallsCtx {
    pub paths: WallsPaths,
    pub config: Config,
    pub secrets: Secrets,
    pub state: State,
    pub provider_status_report: ProviderStatusReport,
}
