use std::sync::{Arc, Mutex as SMutex};

use protocol::{connection::Reader, frame::ServerFrame};
use tokio::sync::oneshot::{self, Receiver, Sender};

use crate::game_state::GameState;

/// Listens to server broadcasts and updates local game state
pub struct Listener {
    game: Arc<SMutex<GameState>>,
    reader: Reader,
    sender: Sender<Result<(), ()>>,
}

impl Listener {
    pub fn new(game: Arc<SMutex<GameState>>, reader: Reader) -> (Self, Receiver<Result<(), ()>>) {
        let (sender, receiver) = oneshot::channel();
        let listener = Self {
            game,
            reader,
            sender,
        };

        (listener, receiver)
    }

    pub async fn run(mut self) {
        loop {
            match self.reader.read_frame::<ServerFrame>().await {
                Ok(frame) => {
                    println!("Got server frame: {:?}", frame);
                    match frame {
                        ServerFrame::GameEnd => {
                            // Game has ended successfully
                            self.sender.send(Ok(())).unwrap();
                            return;
                        }
                        ServerFrame::StateUpdate(dynamic_game_state) => {
                            // Update the game state for rendering
                            self.game.lock().unwrap().update(dynamic_game_state);
                        }
                        _ => {
                            // Unexpected message for this state, so end the game with an error
                            self.sender.send(Err(())).unwrap();
                            return;
                        }
                    }
                }
                Err(_) => {
                    // Treat an error like an end game
                    self.sender.send(Err(())).unwrap();
                    return;
                }
            }
        }
    }
}
