use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("{message} at {location}")]
    MissingProject {
        message: &'static str,
        project_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    MissingElement {
        message: &'static str,
        symbol_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    MissingSymbol {
        message: &'static str,
        symbol_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    MissingSpan {
        message: &'static str,
        span_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    UnsupportedFormat {
        message: &'static str,
        format: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    AmbiguousSymbol {
        message: &'static str,
        symbol_id: String,
        project_ids: Vec<String>,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    SerializationFailed {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    GeneratedScipMetadataReadFailed {
        message: &'static str,
        path: String,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    TemporaryScipOutputCreationFailed {
        message: &'static str,
        path: String,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ReportGenerationFailed {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ServerRuntime {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
}

impl McpError {
    #[track_caller]
    pub fn missing_project(project_id: impl Into<String>) -> Self {
        Self::MissingProject {
            message: "requested RefactorRadar project was not found",
            project_id: project_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn missing_element(symbol_id: impl Into<String>) -> Self {
        Self::MissingElement {
            message: "requested RefactorRadar element was not found",
            symbol_id: symbol_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn missing_symbol(symbol_id: impl Into<String>) -> Self {
        Self::MissingSymbol {
            message: "requested RefactorRadar symbol was not found",
            symbol_id: symbol_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn missing_span(span_id: impl Into<String>) -> Self {
        Self::MissingSpan {
            message: "requested RefactorRadar source span was not found",
            span_id: span_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            message: "unsupported SCIP format",
            format: format.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn ambiguous_symbol(symbol_id: impl Into<String>, project_ids: Vec<String>) -> Self {
        Self::AmbiguousSymbol {
            message: "symbol id matched more than one cached project",
            symbol_id: symbol_id.into(),
            project_ids,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn serialization_failed(details: impl Into<String>) -> Self {
        Self::SerializationFailed {
            message: "failed to serialize MCP tool result",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn generated_scip_metadata_read_failed(
        path: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::GeneratedScipMetadataReadFailed {
            message: "failed to read generated SCIP file metadata",
            path: path.into(),
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn temporary_scip_output_creation_failed(
        path: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::TemporaryScipOutputCreationFailed {
            message: "failed to create temporary SCIP output file",
            path: path.into(),
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn report_generation_failed(details: impl Into<String>) -> Self {
        Self::ReportGenerationFailed {
            message: "RefactorRadar report generation failed",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn server_runtime(details: impl Into<String>) -> Self {
        Self::ServerRuntime {
            message: "RefactorRadar MCP server failed",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::MissingProject { message, .. }
            | Self::MissingElement { message, .. }
            | Self::MissingSymbol { message, .. }
            | Self::MissingSpan { message, .. }
            | Self::UnsupportedFormat { message, .. }
            | Self::AmbiguousSymbol { message, .. }
            | Self::SerializationFailed { message, .. }
            | Self::GeneratedScipMetadataReadFailed { message, .. }
            | Self::TemporaryScipOutputCreationFailed { message, .. }
            | Self::ReportGenerationFailed { message, .. }
            | Self::ServerRuntime { message, .. } => message,
        }
    }
}
