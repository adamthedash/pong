use protocol::frame::{DynamicGameState, FixedGameState, InitialGameState};

/// Client-side copy of the game state
pub struct GameState {
    pub fixed: FixedGameState,
    pub dynamic: DynamicGameState,
}

impl GameState {
    pub fn from_initial_frame(state: InitialGameState) -> Self {
        Self {
            fixed: state.fixed,
            dynamic: state.dynamic,
        }
    }

    pub fn update(&mut self, state: DynamicGameState) {
        self.dynamic = state;
    }
}
