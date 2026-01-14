use std::sync::{Arc, Mutex as SMutex};

use protocol::{connection::Reader, frame::ServerFrame};

use crate::game_state::GameState;

#[derive(Debug)]
pub enum Error {
    Protocol(protocol::connection::Error),
    UnexpectedFrame(ServerFrame),
}

/// Listens to server broadcasts and updates local game state
pub struct Listener {
    game: Arc<SMutex<GameState>>,
    reader: Reader,
}

impl Listener {
    pub fn new(game: Arc<SMutex<GameState>>, reader: Reader) -> Self {
        Self { game, reader }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            let frame = self
                .reader
                .read_frame::<ServerFrame>()
                .await
                .map_err(Error::Protocol)?;

            log::debug!("Got server frame: {:?}", frame);
            match frame {
                ServerFrame::StateUpdate(dynamic_game_state) => {
                    // Update the game state for rendering
                    self.game.lock().unwrap().update(dynamic_game_state);
                }
                ServerFrame::GameEnd => {
                    // Game has ended successfully
                    return Ok(());
                }
                frame => {
                    // Unexpected message for this state, so end the game with an error
                    return Err(Error::UnexpectedFrame(frame));
                }
            }
        }
    }
}
