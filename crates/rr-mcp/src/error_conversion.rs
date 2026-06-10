use crate::McpError;

use rr_scip::ScipError;

use rmcp::ErrorData as ProtocolError;
use serde_json::json;

impl From<McpError> for ProtocolError {
    fn from(error: McpError) -> Self {
        to_protocol_error(error)
    }
}

pub fn to_protocol_error(error: McpError) -> ProtocolError {
    match error {
        McpError::MissingProject {
            message,
            project_id,
            ..
        } => ProtocolError::resource_not_found(message, Some(json!({ "project_id": project_id }))),
        McpError::MissingElement {
            message, symbol_id, ..
        }
        | McpError::MissingSymbol {
            message, symbol_id, ..
        } => ProtocolError::resource_not_found(message, Some(json!({ "symbol_id": symbol_id }))),
        McpError::MissingSpan {
            message, span_id, ..
        } => ProtocolError::resource_not_found(message, Some(json!({ "span_id": span_id }))),
        McpError::UnsupportedFormat {
            message, format, ..
        } => ProtocolError::invalid_params(message, Some(json!({ "format": format }))),
        McpError::AmbiguousSymbol {
            message,
            symbol_id,
            project_ids,
            ..
        } => ProtocolError::invalid_params(
            message,
            Some(json!({ "symbol_id": symbol_id, "project_ids": project_ids })),
        ),
        McpError::SerializationFailed {
            message, details, ..
        }
        | McpError::ReportGenerationFailed {
            message, details, ..
        }
        | McpError::ServerRuntime {
            message, details, ..
        } => ProtocolError::internal_error(message, Some(json!({ "details": details }))),
        McpError::GeneratedScipMetadataReadFailed {
            message,
            path,
            details,
            ..
        }
        | McpError::TemporaryScipOutputCreationFailed {
            message,
            path,
            details,
            ..
        } => ProtocolError::internal_error(
            message,
            Some(json!({ "path": path, "details": details })),
        ),
    }
}

pub fn scip_to_protocol_error(error: ScipError) -> ProtocolError {
    match error {
        ScipError::UnknownFormat { message, path, .. } => {
            ProtocolError::invalid_params(message, Some(json!({ "path": path })))
        }
        ScipError::RustAnalyzerMissing {
            message,
            executable,
            ..
        } => ProtocolError::internal_error(message, Some(json!({ "executable": executable }))),
        ScipError::RustAnalyzerLaunchFailed {
            message,
            executable,
            details,
            ..
        } => ProtocolError::internal_error(
            message,
            Some(json!({ "executable": executable, "details": details })),
        ),
        ScipError::RustAnalyzerFailed {
            message,
            status,
            stderr,
            ..
        } => ProtocolError::internal_error(
            message,
            Some(json!({ "status": status, "stderr": stderr })),
        ),
        ScipError::ReadFailed { message, path, .. }
        | ScipError::ParseFailed { message, path, .. } => {
            ProtocolError::internal_error(message, Some(json!({ "path": path })))
        }
        ScipError::CoreProjectionFailed { message, .. } => {
            ProtocolError::internal_error(message, None)
        }
    }
}
