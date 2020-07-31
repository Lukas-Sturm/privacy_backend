use crate::round_data::RoundData;
use serde_derive::*;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum FromPlayerMessage {
    Initialize { name: String },
    Answer { yes: bool },
    Guess { number: String },
    Disconnect,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum ToPlayerMessage {
    PlayerList {
        players: Vec<String>,
        has_answered: Vec<String>,
    },
    RoundData {
        data: RoundData,
    },
    PoseQuestion {
        question: String,
    },
}
