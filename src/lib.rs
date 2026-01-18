pub mod cli;

mod config;
mod diff;
mod digest;
mod doctor;
mod error;
mod i18n;
mod skill;
mod status;
mod sync;

pub use config::{Config, Target};
pub use doctor::{DoctorReport, doctor_root, group_issues_by_skill};
pub use error::{AppError, AppResult};
pub use skill::validate_skill_id;

#[cfg(test)]
mod core_e2e_tests;
