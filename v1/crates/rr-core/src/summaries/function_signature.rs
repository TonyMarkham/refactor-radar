use crate::{FunctionParameterSummary, TypeReferenceSummary};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub signature_text: Option<String>,
    pub parameters: Vec<FunctionParameterSummary>,
    pub return_type: Option<TypeReferenceSummary>,
    pub confirmed: Vec<String>,
    pub unknown: Vec<String>,
}
