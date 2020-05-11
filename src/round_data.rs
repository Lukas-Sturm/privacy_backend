use serde_derive::*;
use std::collections::HashMap;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct RoundData {
    pub question: String,

    pub yes: usize,
    pub no: usize,

    pub guesses: HashMap<String, String>,
}
