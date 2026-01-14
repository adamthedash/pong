use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};

use egui::{InputState, Key};
use protocol::{connection::Writer, frame::ClientFrame};
use tokio::sync::Mutex;

/// Takes user input and sends them off to the server
pub struct Controller {
    writer: Arc<Mutex<Writer>>,
    receiver: Receiver<InputState>,
    /// Cache last sent frame so we don't bombard server
    last_frame: ClientFrame,
}

impl Controller {
    pub fn new(writer: Arc<Mutex<Writer>>) -> (Self, Sender<InputState>) {
        // TODO: use tokio channel so we don't block thread
        let (sender, receiver) = mpsc::channel();
        let controller = Self {
            writer,
            receiver,
            last_frame: ClientFrame::PaddleStop,
        };

        (controller, sender)
    }

    pub async fn run(mut self) {
        // TODO: deal with errors in channel / writer
        while let Ok(inputs) = self.receiver.recv() {
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
                    .unwrap();

                self.last_frame = paddle_dir;
            }
        }
    }
}
