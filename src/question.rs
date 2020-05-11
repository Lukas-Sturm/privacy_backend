use serde_derive::*;
use std::collections::HashSet;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct Question {
    pub text: String,

    #[serde(default)]
    pub tags: HashSet<String>,
}
