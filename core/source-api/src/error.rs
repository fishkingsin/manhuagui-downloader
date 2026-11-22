#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    Network { message: String },
    RateLimited { message: String },
    ParseFailed { message: String },
    NotFound { message: String },
    Cancelled { message: String },
}

impl From<SourceError> for eyre::Report {
    fn from(error: SourceError) -> Self {
        let message = match error {
            SourceError::Network { message }
            | SourceError::RateLimited { message }
            | SourceError::ParseFailed { message }
            | SourceError::NotFound { message }
            | SourceError::Cancelled { message } => message,
        };
        eyre::eyre!("{message}")
    }
}
