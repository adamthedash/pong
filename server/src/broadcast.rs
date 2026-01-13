use std::{
    sync::{Arc, Mutex as SMutex},
    time::Duration,
};

use protocol::{
    connection::Writer,
    frame::{DynamicGameState, FixedGameState, InitialGameState, ServerFrame},
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

    fn prepare_initial_state(&self) -> InitialGameState {
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

    /// Broadcast a game state update to all players
    async fn broadcast(&self) {
        let state = self.prepare_game_state();
        let frame = ServerFrame::StateUpdate(state);

        // TODO: Broadcast to everyone in parallel
        for writer in &self.writers {
            writer.lock().await.write_frame(&frame).await.unwrap();
        }
    }

    /// Broadcast that the game is beginning and the initial state
    pub async fn signal_begin(&self) {
        let state = self.prepare_initial_state();
        let frame = ServerFrame::GameStart(state);

        // TODO: Broadcast to everyone in parallel
        for writer in &self.writers {
            writer.lock().await.write_frame(&frame).await.unwrap();
        }
    }

    pub async fn signal_end(&self) {
        let frame = ServerFrame::GameEnd;

        // TODO: Broadcast to everyone in parallel
        for writer in &self.writers {
            writer.lock().await.write_frame(&frame).await.unwrap();
        }
    }

    /// Start broadcasting the game state
    pub async fn run(&mut self) {
        self.signal_begin().await;
        log::debug!("signalled begin");

        loop {
            self.broadcast().await;
            log::debug!("Broadcasted state");
            sleep(Duration::from_secs_f32(1. / self.broadcast_rate)).await;
        }
    }
}
