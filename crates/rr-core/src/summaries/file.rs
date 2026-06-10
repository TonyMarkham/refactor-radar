use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub document_path: String,
    pub language: String,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
