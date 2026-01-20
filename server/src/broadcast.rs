use std::{
    sync::{Arc, Mutex as SMutex},
    time::Duration,
};

use futures::future::join_all;
use protocol::{
    connection::Writer,
    frame::{DynamicGameState, FixedGameState, Frame, InitialGameState, ServerFrame},
};
use tokio::{sync::Mutex as TMutex, time::sleep};

use crate::game_state::GameState;

pub struct BroadcastThread {
    game: Arc<SMutex<GameState>>,
    // Connected clients
    pub writers: Vec<Arc<TMutex<Writer>>>,
    // How often the game state is sent to clients, in broadcasts/second
    broadcast_rate: f32,
}

impl BroadcastThread {
    pub fn new(game: Arc<SMutex<GameState>>, broadcast_rate: f32) -> Self {
        Self {
            game,
            writers: vec![],
            broadcast_rate,
        }
    }

    fn prepare_game_state(&self) -> DynamicGameState {
        let g = self.game.lock().unwrap();

        DynamicGameState {
            left_paddle_y: g.left_paddle_pos[0] as u32,
            right_paddle_y: g.right_paddle_pos[0] as u32,
            ball_position: (g.ball_pos[0] as u32, g.ball_pos[1] as u32),
            ball_direction: (g.ball_dir[0] as i8, g.ball_dir[1] as i8),
            score_left: g.score.0,
            score_right: g.score.1,
        }
    }

    fn prepare_full_state(&self) -> InitialGameState {
        let dynamic = self.prepare_game_state();

        let g = self.game.lock().unwrap();

        let fixed = FixedGameState {
            left_paddle_x: g.left_paddle_pos[1] as u32,
            right_paddle_x: g.right_paddle_pos[1] as u32,
            paddle_height: g.paddle_size,
            arena_size: (g.arena_size[0] as u32, g.arena_size[1] as u32),
        };

        InitialGameState { fixed, dynamic }
    }

    /// Broadcast a frame to all clients in parallel
    async fn broadcast<F: Frame>(&self, frame: &F) -> Vec<Result<(), protocol::connection::Error>> {
        join_all(
            self.writers
                .iter()
                .map(|writer| async { writer.lock().await.write_frame(frame).await }),
        )
        .await
    }

    /// Broadcast a game state update to all players
    async fn broadcast_state(&self) {
        let state = self.prepare_game_state();
        let frame = ServerFrame::StateUpdate(state);

        self.broadcast(&frame)
            .await
            .into_iter()
            .filter_map(Result::err)
            .for_each(|err| {
                log::error!("Error broadcasting state: {:?}", err);
            });
    }

    /// Broadcast that the game is beginning and the initial state
    pub async fn signal_begin(&self) {
        let state = self.prepare_full_state();
        let frame = ServerFrame::GameStart(state);

        self.broadcast(&frame)
            .await
            .into_iter()
            .filter_map(Result::err)
            .for_each(|err| {
                log::error!("Error broadcasting game start: {:?}", err);
            });
    }

    /// Broadcast that the game has ended
    pub async fn signal_end(&self) {
        let frame = ServerFrame::GameEnd;

        self.broadcast(&frame)
            .await
            .into_iter()
            .filter_map(Result::err)
            .for_each(|err| {
                log::error!("Error broadcasting game end: {:?}", err);
            });
    }

    /// Start broadcasting the game state
    pub async fn run(&self) {
        self.signal_begin().await;
        log::debug!("signalled begin");

        loop {
            self.broadcast_state().await;
            log::debug!("Broadcasted state");
            sleep(Duration::from_secs_f32(1. / self.broadcast_rate)).await;
        }
    }
}
