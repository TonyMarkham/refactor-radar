use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Stable(String);

impl Stable {
    pub fn from_scip_symbol(symbol: &str) -> Self {
        Self(format!("scip:{symbol}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
