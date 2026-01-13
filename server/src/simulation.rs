use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::game_state::GameState;

/// Responsible for simulating the game state
pub struct GameThread {
    game: Arc<Mutex<GameState>>,
    // How often the game simulation is advanced, in ticks/second
    tick_rate: f32,
}

impl GameThread {
    pub fn new(game: Arc<Mutex<GameState>>, tick_rate: f32) -> Self {
        Self { game, tick_rate }
    }

    pub async fn run(&mut self) {
        loop {
            // TODO: Account for time taken to simulate
            tokio::time::sleep(Duration::from_secs_f32(1. / self.tick_rate)).await;

            self.game
                .lock()
                .unwrap()
                .tick(&Duration::from_secs_f32(1. / self.tick_rate));
        }
    }
}
