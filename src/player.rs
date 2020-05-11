use crate::message::{FromPlayerMessage, ToPlayerMessage};
use async_std::net::SocketAddr;
use async_std::net::TcpStream;
use async_tungstenite::WebSocketStream;
use futures::channel::mpsc::*;
use futures::*;
use tungstenite::Message;

pub struct Player {
    pub name: String,
    pub stream: WebSocketStream<TcpStream>,
    pub address: SocketAddr,
}

impl Player {
    pub async fn accept(stream: TcpStream, address: SocketAddr) -> Option<Self> {
        let mut stream = async_tungstenite::accept_async(stream).await.ok()?;

        let message = { Self::receive_from_player_message(&mut stream).await? };

        return match message {
            FromPlayerMessage::Initialize { name } => Some(Player {
                name,
                stream,
                address,
            }),
            _ => None,
        };
    }

    async fn receive_from_player_message(
        stream: &mut WebSocketStream<TcpStream>,
    ) -> Option<FromPlayerMessage> {
        loop {
            let message = stream.next().await?.ok()?;

            match message {
                Message::Text(text) => {
                    let message: FromPlayerMessage = serde_json::from_str(&text).ok()?;

                    return Some(message);
                }
                _ => (),
            }
        }
    }

    pub async fn receive_message(&mut self) -> Option<FromPlayerMessage> {
        Self::receive_from_player_message(&mut self.stream).await
    }

    pub async fn send_message(&mut self, message: ToPlayerMessage) -> Option<()> {
        let value = serde_json::to_string(&message).ok()?;
        self.stream.send(Message::Text(value)).await.ok()
    }

    pub async fn run(
        mut self,
        to_game: UnboundedSender<(String, FromPlayerMessage)>,
        mut from_game: UnboundedReceiver<ToPlayerMessage>,
    ) {
        loop {
            let mut from_player = self.stream.next().fuse();
            let mut to_player = from_game.next();

            let result = select! {
                from_player = from_player => {
                    from_player
                        .and_then(|v| v.ok())
                        .and_then(|message| {
                            match message {
                                Message::Text(text) => {
                                    let message: FromPlayerMessage = serde_json::from_str(&text).ok()?;
                                    to_game.unbounded_send((
                                        self.name.clone(),
                                        message,
                                    )).ok()
                                },
                                _ => return Some(())
                            }
                    })
                },
                to_player = to_player => {
                    match to_player {
                        Some(message) => {
                            self.send_message(message).await
                        }
                        None => None,
                    }
                },
            };

            if result.is_none() {
                let _ = to_game.unbounded_send((self.name.clone(), FromPlayerMessage::Disconnect));
                return;
            }
        }
    }
}
