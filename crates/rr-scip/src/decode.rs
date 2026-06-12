use crate::{Index, ScipError, ScipResult};
use prost::Message;
use std::{fs, path::PathBuf};

#[track_caller]
pub fn decode_scip_from_path(path: PathBuf) -> ScipResult<Index> {
    let bytes = fs::read(&path).map_err(|source| ScipError::read(path.clone(), source))?;

    decode_scip_from_bytes(bytes)
}

#[track_caller]
pub fn decode_scip_from_bytes(bytes: impl AsRef<[u8]>) -> ScipResult<Index> {
    Index::decode(bytes.as_ref()).map_err(ScipError::decode)
}
