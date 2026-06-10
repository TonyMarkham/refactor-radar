use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FiveW {
    pub who: Vec<String>,
    pub what: Vec<String>,
    #[serde(rename = "where")]
    pub where_: Vec<String>,
    pub when: Vec<String>,
    pub why: Vec<String>,
    pub how: Vec<String>,
}
