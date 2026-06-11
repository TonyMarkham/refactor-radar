use crate::GeneratedFiveW;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedBriefContent {
    pub summary: String,
    pub five_w: GeneratedFiveW,
    pub unknowns: Vec<String>,
}
