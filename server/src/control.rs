use std::sync::{Arc, Mutex};

use protocol::{
    connection::Reader,
    frame::{ClientFrame, Paddle},
};

use crate::game_state::GameState;

#[derive(Debug)]
pub enum Error {
    InvalidFrame(ClientFrame),
    Protocol(protocol::connection::Error),
}

impl From<protocol::connection::Error> for Error {
    fn from(value: protocol::connection::Error) -> Self {
        Self::Protocol(value)
    }
}

pub struct ControlThread {
    paddle: Paddle,
    game: Arc<Mutex<GameState>>,
    reader: Reader,
}

impl ControlThread {
    pub fn new(game: Arc<Mutex<GameState>>, reader: Reader, paddle: Paddle) -> Self {
        Self {
            paddle,
            game,
            reader,
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        loop {
            match self.reader.read_frame().await? {
                ClientFrame::PaddleUp => {
                    let mut g = self.game.lock().unwrap();
                    match self.paddle {
                        Paddle::Left => g.left_paddle_dir = 1,
                        Paddle::Right => g.right_paddle_dir = 1,
                    }
                }
                ClientFrame::PaddleDown => {
                    let mut g = self.game.lock().unwrap();
                    match self.paddle {
                        Paddle::Left => g.left_paddle_dir = -1,
                        Paddle::Right => g.right_paddle_dir = -1,
                    }
                }
                ClientFrame::PaddleStop => {
                    let mut g = self.game.lock().unwrap();
                    match self.paddle {
                        Paddle::Left => g.left_paddle_dir = 0,
                        Paddle::Right => g.right_paddle_dir = 0,
                    }
                }
                ClientFrame::Disconnect => return Ok(()),

                // Invalid command
                frame => return Err(Error::InvalidFrame(frame)),
            }
        }
    }
}
