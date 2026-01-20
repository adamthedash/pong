use protocol::connection::Reader;
use protocol::connection::Writer;
use quinn::Connection;
use std::sync::Arc;
use std::sync::Mutex as SMutex;
use tokio::sync::Mutex as TMutex;

use protocol::frame::ClientFrame;
use protocol::frame::Paddle;
use protocol::frame::ServerFrame;

use crate::{
    broadcast::BroadcastThread, control::ControlThread, game_state::GameState,
    simulation::GameThread,
};

#[derive(Debug)]
pub enum Error {
    InvalidFrame(ClientFrame),
    Protocol(protocol::connection::Error),
    Quinn(quinn::ConnectionError),
    ServerFull,
}

impl From<protocol::connection::Error> for Error {
    fn from(value: protocol::connection::Error) -> Self {
        Self::Protocol(value)
    }
}

impl From<quinn::ConnectionError> for Error {
    fn from(value: quinn::ConnectionError) -> Self {
        Self::Quinn(value)
    }
}

pub struct GameServer {
    game: Arc<SMutex<GameState>>,
    simulator: Arc<TMutex<GameThread>>,
    broadcaster: Arc<TMutex<BroadcastThread>>,
    left_controller: Option<ControlThread>,
    right_controller: Option<ControlThread>,
}

impl GameServer {
    pub fn new(broadcast_rate: f32, tick_rate: f32) -> Self {
        let game = Arc::new(SMutex::new(GameState::default()));
        let simulator = Arc::new(TMutex::new(GameThread::new(game.clone(), tick_rate)));
        let broadcaster = Arc::new(TMutex::new(BroadcastThread::new(
            game.clone(),
            broadcast_rate,
        )));

        Self {
            game,
            simulator,
            broadcaster,
            left_controller: None,
            right_controller: None,
        }
    }

    // Attempt to add a player to the game
    pub async fn connect_player(&mut self, connection: Connection) -> Result<(), Error> {
        let (send, recv) = connection.accept_bi().await?;
        let mut writer = Writer::new(send);
        let mut reader = Reader::new(recv);

        // Perform 3-way handshake
        match reader.read_frame::<ClientFrame>().await? {
            ClientFrame::Connect => (),
            frame => return Err(Error::InvalidFrame(frame)),
        }
        log::debug!("Got connect frame");

        let paddle = match (&self.left_controller, &self.right_controller) {
            (None, _) => Paddle::Left,
            (Some(_), None) => Paddle::Right,
            (Some(_), Some(_)) => {
                writer
                    .write_frame(&ServerFrame::RejectConnection("Server is full".to_string()))
                    .await?;
                return Err(Error::ServerFull);
            }
        };
        writer
            .write_frame(&ServerFrame::AcceptConnection(paddle))
            .await?;
        log::debug!("Accepting connection for paddle: {:?}", paddle);

        match reader.read_frame::<ClientFrame>().await? {
            ClientFrame::ConnectedAck => (),
            frame => return Err(Error::InvalidFrame(frame)),
        }
        log::debug!("Got connect ack");

        // Create control thread
        let controller = ControlThread::new(self.game.clone(), reader, paddle);
        match paddle {
            Paddle::Left => self.left_controller = Some(controller),
            Paddle::Right => self.right_controller = Some(controller),
        }

        // Add writer to broadcaster
        self.broadcaster
            .lock()
            .await
            .writers
            .push(Arc::new(TMutex::new(writer)));

        Ok(())
    }

    pub fn ready(&self) -> bool {
        self.left_controller.is_some() && self.right_controller.is_some()
    }

    pub async fn run(&mut self) {
        assert!(self.ready(), "Both players must be connected");

        // Start the simulator in the background
        let simulator = tokio::spawn({
            let simulator = self.simulator.clone();
            async move { simulator.lock().await.run().await }
        });
        log::info!("Simulator started");

        // Start broadcaster in the background & signal game start
        let broadcaster = tokio::spawn({
            let broadcaster = self.broadcaster.clone();
            async move { broadcaster.lock().await.run().await }
        });
        log::info!("Broadcaster started");

        // Start controllers to accept commands
        let left_controller = self.left_controller.as_mut().unwrap();
        let right_controller = self.right_controller.as_mut().unwrap();

        let result = tokio::select! {
            e = left_controller.run() => e,
            e = right_controller.run() => e,
        };

        if let Err(e) = result {
            log::error!("Game stopped due to error: {e:?}");
        }

        // Signal to players that game has ended
        broadcaster.abort();
        self.broadcaster.lock().await.signal_end().await;

        // Stop game simulation
        simulator.abort();
    }
}
