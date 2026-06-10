use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{message} at {location}")]
    InvalidScipRange {
        message: &'static str,
        range: Vec<i32>,
        location: ErrorLocation,
    },
}

impl CoreError {
    #[track_caller]
    pub fn invalid_scip_range(range: Vec<i32>) -> Self {
        Self::InvalidScipRange {
            message: "SCIP range must contain valid zero-based positions",
            range,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidScipRange { message, .. } => message,
        }
    }
}
