use crate::message::*;
use crate::question::Question;
use crate::round_data::RoundData;
use futures::channel::mpsc::*;
use futures::*;
use log::*;
use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;

pub struct PlayerState {
    pub sender: UnboundedSender<ToPlayerMessage>,

    pub answer: Option<bool>,
    pub guess: Option<String>,
}

pub struct GameRoom {
    pub previous_rounds: Vec<RoundData>,

    pub players: HashMap<String, PlayerState>,

    receiver: UnboundedReceiver<(String, FromPlayerMessage)>,

    new_player_receiver: UnboundedReceiver<(String, UnboundedSender<ToPlayerMessage>)>,

    questions: Vec<Question>,
    current_question: usize,
}

impl GameRoom {
    pub fn new(
        new_player_receiver: UnboundedReceiver<(String, UnboundedSender<ToPlayerMessage>)>,
        questions: Vec<Question>,
    ) -> (Self, UnboundedSender<(String, FromPlayerMessage)>) {
        let (sender, receiver) = unbounded();

        let mut room = GameRoom {
            previous_rounds: Default::default(),
            players: Default::default(),

            receiver,

            new_player_receiver,

            questions,
            current_question: 0,
        };
        room.shuffle();

        info!(
            "GameRoom started, current question is \"{}\"",
            room.get_question().text.as_str()
        );

        (room, sender)
    }

    /// The game main loop
    pub async fn run(mut self: Pin<&mut Self>) {
        loop {
            let GameRoom {
                previous_rounds: _,
                players: _,
                receiver,
                new_player_receiver,
                questions: _,
                current_question: _,
            } = &mut *self;

            let mut player_message = receiver.next();
            let mut new_player = new_player_receiver.next();

            select! {
                player_message = player_message => {
                    let (player, message) = match player_message {
                        Some(v) => v,
                        _ => return,
                    };
                    self.handle_player_message(player, message);
                },
                new_player = new_player => {
                    let (player, state) = match new_player {
                        Some(v) => v,
                        _ => return,
                    };
                    self.join_player(player, state);
                },
            };

            self.update_state();
        }
    }

    /// React to a message sent from a player
    fn handle_player_message(&mut self, player: String, message: FromPlayerMessage) {
        info!("Received message from {}: {:?}", player, message);

        match message {
            // A player has disconnected
            FromPlayerMessage::Disconnect => {
                let mut dropped_players = HashSet::new();
                dropped_players.insert(player);
                self.handle_dropped_players(dropped_players);
            }

            // The init message
            // It is only used as the first message sent by the client
            FromPlayerMessage::Initialize { .. } => (),

            // The player has set their answer
            FromPlayerMessage::Answer { yes } => {
                if let Some(state) = self.players.get_mut(&player) {
                    state.answer = Some(yes);

                    if state.answer.is_some() && state.guess.is_some() {
                        // We must tell the others about that, the player is ready
                        self.broadcast_players();
                    }
                }
            }

            // The player has set their guess
            FromPlayerMessage::Guess { number } => {
                if let Some(state) = self.players.get_mut(&player) {
                    state.guess = Some(number);

                    if state.answer.is_some() && state.guess.is_some() {
                        // We must tell the others about that, the player is ready
                        self.broadcast_players();
                    }
                }
            }
        }
    }

    /// Update the internal game state
    ///
    /// If all players have answered, move on to the next question
    fn update_state(&mut self) {
        // If no players are in, return
        if self.players.is_empty() {
            return;
        }

        // If not all players have answered, return
        if !self
            .players
            .iter()
            .all(|(_, state)| state.guess.is_some() && state.answer.is_some())
        {
            return;
        }

        let mut round = RoundData {
            question: self.get_question().text.clone(),
            yes: 0,
            no: 0,
            guesses: Default::default(),
        };

        for (player, state) in self.players.iter_mut() {
            if state.answer.take().unwrap() {
                round.yes += 1;
            } else {
                round.no += 1;
            }
            round
                .guesses
                .insert(player.clone(), state.guess.take().unwrap());
        }

        self.next_question();
        self.broadcast(ToPlayerMessage::RoundData { data: round });
        self.broadcast(ToPlayerMessage::PoseQuestion {
            question: self.get_question().text.clone(),
        });
        self.broadcast_players();
    }

    fn next_question(&mut self) {
        self.current_question += 1;
        if self.current_question >= self.questions.len() {
            self.shuffle();
        }

        info!("Current question is {}", self.get_question().text.as_str());
    }

    fn shuffle(&mut self) {
        self.questions.shuffle(&mut thread_rng());
        self.current_question = 0;
    }

    fn get_question(&self) -> &Question {
        self.questions.get(self.current_question).unwrap()
    }

    fn join_player(&mut self, name: String, sender: UnboundedSender<ToPlayerMessage>) {
        if sender
            .unbounded_send(ToPlayerMessage::PoseQuestion {
                question: self.get_question().text.clone(),
            })
            .is_err()
        {
            return;
        }

        let state = PlayerState {
            sender,
            answer: None,
            guess: None,
        };
        self.players.insert(name, state);
        self.broadcast_players();
    }

    /// Broadcast the list of players
    fn broadcast_players(&mut self) {
        let players = self.players.iter().map(|(k, _)| k.clone()).collect();

        let has_answered = self
            .players
            .iter()
            .filter_map(|(k, v)| {
                if v.guess.is_some() && v.answer.is_some() {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();

        self.broadcast(ToPlayerMessage::PlayerList {
            players,
            has_answered,
        })
    }

    /// Broadcast a message to all players
    fn broadcast(&mut self, message: ToPlayerMessage) {
        info!("Broadcast {:?}", message);

        let mut dropped_players = HashSet::new();

        for (player, state) in self.players.iter_mut() {
            let result = state.sender.unbounded_send(message.clone());
            if result.is_err() {
                dropped_players.insert(player.clone());
            }
        }

        self.handle_dropped_players(dropped_players);
    }

    /// Take care of players that have dropped
    ///
    /// The remaining players must be informed
    fn handle_dropped_players(&mut self, dropped_players: HashSet<String>) {
        if dropped_players.is_empty() {
            return;
        }

        self.players.retain(|key, _| !dropped_players.contains(key));
        self.broadcast_players();
    }
}
