use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Config {
        message: String,
        hint: Option<String>,
    },
    #[error("{message}")]
    Exec {
        message: String,
        hint: Option<String>,
    },
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn config(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::Config {
            message: message.into(),
            hint,
        }
    }

    pub fn exec(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::Exec {
            message: message.into(),
            hint,
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Config { hint, .. } | Self::Exec { hint, .. } => hint.as_deref(),
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Config { .. } => ExitCode::from(3),
            Self::Exec { .. } => ExitCode::from(4),
        }
    }
}
