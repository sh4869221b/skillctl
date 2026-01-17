pub mod cli;

mod config;
mod diff;
mod digest;
mod error;
mod i18n;
mod skill;
mod status;
mod sync;

pub use config::{Config, Target};
pub use error::{AppError, AppResult};
pub use skill::validate_skill_id;

#[cfg(test)]
mod integration_tests;
