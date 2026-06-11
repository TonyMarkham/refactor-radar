use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("{message} at {location}")]
    ElementNotFound {
        message: &'static str,
        symbol_id: String,
        location: ErrorLocation,
    },
}

impl ReportError {
    #[track_caller]
    pub fn element_not_found(symbol_id: impl Into<String>) -> Self {
        Self::ElementNotFound {
            message: "element was not found in the projected semantic model",
            symbol_id: symbol_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::ElementNotFound { message, .. } => message,
        }
    }
}
