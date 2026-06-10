use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedFiveW {
    pub who: String,
    pub what: String,
    pub when: String,
    #[serde(rename = "where")]
    pub where_: String,
    pub why: String,
    pub how: String,
}
