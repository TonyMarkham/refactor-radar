use crate::{SourceSpan, TypeReferenceSummary};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub name: String,
    pub scip_symbol: Option<String>,
    pub parameter_kind: String,
    pub source_span: Option<SourceSpan>,
    pub type_reference: Option<TypeReferenceSummary>,
}
