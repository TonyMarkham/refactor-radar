use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Span(String);

impl Span {
    pub fn new(project_id: &str, document_path: &str, line: u32, column: u32) -> Self {
        Self(format!("{project_id}::{document_path}:{line}:{column}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
