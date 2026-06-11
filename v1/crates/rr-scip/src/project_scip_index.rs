use crate::{Format, ScipError, ScipResult, scip_to_core};

use rr_core::SemanticModel;

use protobuf::Message;
use scip::types::Index;
use std::path::{Path, PathBuf};

pub fn project_scip_index(path: &Path, format: Option<Format>) -> ScipResult<SemanticModel> {
    let format = match format {
        Some(format) => format,
        None => Format::detect(path)?,
    };
    let path_buf = PathBuf::from(path);
    let bytes = std::fs::read(path).map_err(|_| ScipError::read_failed(path_buf.clone()))?;
    let index = match format {
        Format::Binary => Index::parse_from_bytes(&bytes).map_err(|_| {
            ScipError::parse_failed(path_buf.clone(), "failed to parse binary SCIP protobuf")
        })?,
        Format::Json => {
            let json = String::from_utf8_lossy(&bytes);
            protobuf_json_mapping::parse_from_str::<Index>(json.as_ref()).map_err(|_| {
                ScipError::parse_failed(path_buf.clone(), "failed to parse SCIP protobuf JSON")
            })?
        }
    };
    scip_to_core(&index)
}
