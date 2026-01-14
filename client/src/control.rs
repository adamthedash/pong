use std::sync::Arc;

use egui::{InputState, Key};
use protocol::{connection::Writer, frame::ClientFrame};
use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender},
};

#[derive(Debug)]
pub enum Error {
    Protocol(protocol::connection::Error),
    ChannelClosed,
}

/// Takes user input and sends them off to the server
pub struct Controller {
    writer: Arc<Mutex<Writer>>,
    /// Receives raw input states from GUI to be processed
    receiver: UnboundedReceiver<InputState>,
    /// Cache last sent frame so we don't bombard server
    last_frame: ClientFrame,
}

impl Controller {
    pub fn new(writer: Arc<Mutex<Writer>>) -> (Self, UnboundedSender<InputState>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let controller = Self {
            writer,
            receiver,
            last_frame: ClientFrame::PaddleStop,
        };

        (controller, sender)
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            let inputs = self.receiver.recv().await.ok_or(Error::ChannelClosed)?;

            // Convert inputs to message to be sent
            let paddle_dir = match (
                inputs.key_down(Key::ArrowUp),
                inputs.key_down(Key::ArrowDown),
            ) {
                (true, false) => ClientFrame::PaddleUp,
                (false, true) => ClientFrame::PaddleDown,
                _ => ClientFrame::PaddleStop,
            };

            if paddle_dir != self.last_frame {
                log::debug!("Sending command to server: {:?}", paddle_dir);
                self.writer
                    .lock()
                    .await
                    .write_frame(&paddle_dir)
                    .await
                    .map_err(Error::Protocol)?;

                self.last_frame = paddle_dir;
            }
        }
    }
}
