use std::net::SocketAddr;

use protocol::connection::Connection;

#[derive(Debug)]
pub enum Error {
    MaxPlayersExceeded,
    PlayerAlreadyExists,
    PlayerDoesntExist,
}

pub struct ConnectionPool {
    connections: Vec<Option<SocketAddr>>,
}

impl ConnectionPool {
    pub fn new(max_players: usize) -> Self {
        Self {
            connections: std::iter::repeat_with(|| None).take(max_players).collect(),
        }
    }

    /// Attempt to connect a player to the game
    pub fn try_connect(&mut self, conn: &Connection) -> Result<usize, Error> {
        // First check if this player has already connected
        if self
            .connections
            .iter()
            .filter_map(|x| x.as_ref())
            .any(|c2| conn == c2)
        {
            return Err(Error::PlayerAlreadyExists);
        }

        // Then check if there's any room
        let Some(pos) = self.connections.iter().position(|c| c.is_none()) else {
            return Err(Error::MaxPlayersExceeded);
        };

        // Connect the player
        self.connections[pos] = Some(conn.address());

        Ok(pos)
    }

    /// Attempt to disconnect a player from the game
    pub fn try_disconnect(&mut self, player: usize) -> Result<(), Error> {
        let Some(conn) = self.connections.get_mut(player) else {
            return Err(Error::PlayerDoesntExist);
        };

        if conn.is_none() {
            return Err(Error::PlayerDoesntExist);
        }

        *conn = None;

        Ok(())
    }
}
