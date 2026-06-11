use crate::{ScipError, ScipResult};

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Binary,
    Json,
}

impl Format {
    pub fn detect(path: &Path) -> ScipResult<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("scip") => Ok(Self::Binary),
            Some("json") => Ok(Self::Json),
            _ => Err(ScipError::unknown_format(PathBuf::from(path))),
        }
    }
}
