use error_location::ErrorLocation;
use std::{panic::Location, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScipError {
    #[error("{message} at {location}")]
    UnknownFormat {
        message: &'static str,
        path: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    RustAnalyzerMissing {
        message: &'static str,
        executable: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    RustAnalyzerLaunchFailed {
        message: &'static str,
        executable: PathBuf,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    RustAnalyzerFailed {
        message: &'static str,
        status: Option<i32>,
        stderr: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ReadFailed {
        message: &'static str,
        path: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ParseFailed {
        message: &'static str,
        path: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    CoreProjectionFailed {
        message: &'static str,
        location: ErrorLocation,
    },
}

impl ScipError {
    #[track_caller]
    pub fn unknown_format(path: PathBuf) -> Self {
        Self::UnknownFormat {
            message: "SCIP input format could not be detected from extension",
            path,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn rust_analyzer_missing(executable: PathBuf) -> Self {
        Self::RustAnalyzerMissing {
            message: "rust-analyzer executable was not found",
            executable,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn rust_analyzer_launch_failed(executable: PathBuf, details: impl Into<String>) -> Self {
        Self::RustAnalyzerLaunchFailed {
            message: "rust-analyzer executable could not be launched",
            executable,
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn rust_analyzer_failed(status: Option<i32>, stderr: String) -> Self {
        Self::RustAnalyzerFailed {
            message: "rust-analyzer scip exited unsuccessfully",
            status,
            stderr,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn read_failed(path: PathBuf) -> Self {
        Self::ReadFailed {
            message: "failed to read SCIP input file",
            path,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn parse_failed(path: PathBuf, message: &'static str) -> Self {
        Self::ParseFailed {
            message,
            path,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn core_projection_failed() -> Self {
        Self::CoreProjectionFailed {
            message: "failed to project SCIP source range",
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::UnknownFormat { message, .. }
            | Self::RustAnalyzerMissing { message, .. }
            | Self::RustAnalyzerLaunchFailed { message, .. }
            | Self::RustAnalyzerFailed { message, .. }
            | Self::ReadFailed { message, .. }
            | Self::ParseFailed { message, .. }
            | Self::CoreProjectionFailed { message, .. } => message,
        }
    }
}
