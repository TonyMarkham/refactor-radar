use error_location::ErrorLocation;
use std::{io, panic::Location, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScipError {
    #[error("failed to read SCIP file {path:?} at {location}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
        location: ErrorLocation,
    },

    #[error("failed to decode SCIP file at {location}")]
    Decode {
        #[source]
        source: prost::DecodeError,
        location: ErrorLocation,
    },
}

impl ScipError {
    #[track_caller]
    pub fn read(path: PathBuf, source: io::Error) -> Self {
        Self::Read {
            path,
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn decode(source: prost::DecodeError) -> Self {
        Self::Decode {
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Read { .. } => "failed to read SCIP file",
            Self::Decode { .. } => "failed to decode SCIP file",
        }
    }

    pub fn location(&self) -> ErrorLocation {
        match self {
            Self::Read { location, .. } | Self::Decode { location, .. } => *location,
        }
    }
}
