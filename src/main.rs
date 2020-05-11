use crate::game::GameRoom;
use crate::player::Player;
use crate::question::Question;
use async_std::net::TcpListener;
use futures::channel::mpsc::*;
use futures::executor::block_on;
use futures::*;
use log::*;
use walkdir::WalkDir;

pub mod game;
pub mod message;
pub mod player;
pub mod question;
pub mod round_data;

pub async fn run_server(questions: Vec<Question>) {
    let listener = TcpListener::bind("127.0.0.1:8001")
        .await
        .expect("Could not open socket");

    let (new_player_sender, new_player_receiver) = unbounded();

    let (game, player_message_sender) = GameRoom::new(new_player_receiver, questions);

    async_std::task::spawn(async move {
        pin_mut!(game);
        game.run().await;
    });

    while let Ok((stream, addr)) = listener.accept().await {
        let new_player_sender = new_player_sender.clone();
        let player_message_sender = player_message_sender.clone();

        async_std::task::spawn(async move {
            let player = match Player::accept(stream, addr).await {
                Some(v) => v,
                None => return,
            };

            info!("{} ({}) connected", player.name, player.address);

            let (to_player_sender, to_player_receiver) = unbounded();

            if new_player_sender
                .unbounded_send((player.name.clone(), to_player_sender))
                .is_err()
            {
                return;
            }

            player.run(player_message_sender, to_player_receiver).await;
        });
    }
}

fn load_questions() -> Vec<Question> {
    let mut result = vec![];

    for file in WalkDir::new("questions")
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|entry| entry.file_type().is_file())
    {
        let contents = std::fs::read_to_string(file.path()).unwrap();
        let extension = match file.path().extension().map(|e| e.to_string_lossy()) {
            Some(v) => v,
            _ => continue,
        };
        let extension: &str = &extension;

        if extension == "ron" {
            let mut contents: Vec<Question> = ron::de::from_str(&contents).unwrap();
            result.append(&mut contents);
        }
        if extension == "lines" {
            contents.lines().filter(|l| !l.is_empty()).for_each(|line| {
                result.push(Question {
                    text: line.to_string(),
                    tags: Default::default(),
                });
            })
        }
    }

    info!("Loaded {} questions", result.len());

    result
}

fn main() {
    env_logger::init();

    let questions = load_questions();

    block_on(run_server(questions));
}
