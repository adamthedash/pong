use std::io::{Cursor, Read};

use bytes::{Buf, TryGetError};
use tokio::io::{AsyncWrite, AsyncWriteExt};

#[derive(Debug)]
pub enum Error {
    Incomplete,
    Invalid,
}

impl From<TryGetError> for Error {
    fn from(_: TryGetError) -> Self {
        Error::Incomplete
    }
}

pub trait Frame: Sized {
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self, Error>;

    async fn write<W>(&self, buf: &mut W) -> Result<(), std::io::Error>
    where
        W: AsyncWrite + Unpin;
}

/// Strings encoded as length + utf8 bytes
impl Frame for String {
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self, Error> {
        let length = buf.try_get_u64()? as usize;
        let mut string = vec![0; length];
        buf.read_exact(&mut string).map_err(|_| Error::Incomplete)?;
        let string = String::from_utf8(string).map_err(|_| Error::Invalid)?;

        Ok(string)
    }

    async fn write<W>(&self, buf: &mut W) -> Result<(), std::io::Error>
    where
        W: AsyncWrite + Unpin,
    {
        buf.write_u64(self.len() as u64).await?;
        buf.write_all(self.as_bytes()).await?;

        Ok(())
    }
}

/// Sent by client
#[derive(Debug, PartialEq)]
pub enum ClientFrame {
    Connect,
    ConnectedAck,
    PaddleUp,
    PaddleStop,
    PaddleDown,
    Disconnect,
}

impl Frame for ClientFrame {
    /// Attempt to parse this frame from a buffer
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self, Error> {
        let kind = buf.try_get_u8()?;

        let frame = match kind {
            b'y' => ClientFrame::Connect,
            b'c' => ClientFrame::ConnectedAck,
            b'u' => ClientFrame::PaddleUp,
            b's' => ClientFrame::PaddleStop,
            b'd' => ClientFrame::PaddleDown,
            b'n' => ClientFrame::Disconnect,
            _ => return Err(Error::Invalid),
        };

        Ok(frame)
    }

    /// Write this frame to a stream
    async fn write<W>(&self, buf: &mut W) -> Result<(), std::io::Error>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            ClientFrame::Connect => buf.write_u8(b'y').await,
            ClientFrame::ConnectedAck => buf.write_u8(b'c').await,
            ClientFrame::PaddleUp => buf.write_u8(b'u').await,
            ClientFrame::PaddleStop => buf.write_u8(b's').await,
            ClientFrame::PaddleDown => buf.write_u8(b'd').await,
            ClientFrame::Disconnect => buf.write_u8(b'n').await,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Paddle {
    Left,
    Right,
}

#[derive(Debug)]
pub struct DynamicGameState {
    pub left_paddle_y: u32,
    pub right_paddle_y: u32,
    pub ball_position: (u32, u32),
    pub ball_direction: (i8, i8),
    pub score_left: u32,
    pub score_right: u32,
}

/// NOTE: [`DynamicGameState`] has no leading type identifier
impl Frame for DynamicGameState {
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self, Error> {
        Ok(Self {
            left_paddle_y: buf.try_get_u32()?,
            right_paddle_y: buf.try_get_u32()?,
            ball_position: (buf.try_get_u32()?, buf.try_get_u32()?),
            ball_direction: (buf.try_get_i8()?, buf.try_get_i8()?),
            score_left: buf.try_get_u32()?,
            score_right: buf.try_get_u32()?,
        })
    }

    async fn write<W>(&self, buf: &mut W) -> Result<(), std::io::Error>
    where
        W: AsyncWrite + Unpin,
    {
        buf.write_u32(self.left_paddle_y).await?;
        buf.write_u32(self.right_paddle_y).await?;
        buf.write_u32(self.ball_position.0).await?;
        buf.write_u32(self.ball_position.1).await?;
        buf.write_i8(self.ball_direction.0).await?;
        buf.write_i8(self.ball_direction.1).await?;
        buf.write_u32(self.score_left).await?;
        buf.write_u32(self.score_right).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct FixedGameState {
    pub left_paddle_x: u32,
    pub right_paddle_x: u32,
    pub paddle_height: u32,
    pub arena_size: (u32, u32),
}

/// NOTE: [`FixedGameState`] has no leading type identifier
impl Frame for FixedGameState {
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self, Error> {
        Ok(Self {
            left_paddle_x: buf.try_get_u32()?,
            right_paddle_x: buf.try_get_u32()?,
            paddle_height: buf.try_get_u32()?,
            arena_size: (buf.try_get_u32()?, buf.try_get_u32()?),
        })
    }

    async fn write<W>(&self, buf: &mut W) -> Result<(), std::io::Error>
    where
        W: AsyncWrite + Unpin,
    {
        buf.write_u32(self.left_paddle_x).await?;
        buf.write_u32(self.right_paddle_x).await?;
        buf.write_u32(self.paddle_height).await?;
        buf.write_u32(self.arena_size.0).await?;
        buf.write_u32(self.arena_size.1).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct InitialGameState {
    pub fixed: FixedGameState,
    pub dynamic: DynamicGameState,
}

/// Sent by server
#[derive(Debug)]
pub enum ServerFrame {
    AcceptConnection(Paddle),
    RejectConnection(String),
    /// Used for both a fresh start and resume
    GameStart(InitialGameState),
    GamePause,
    StateUpdate(DynamicGameState),
    GameEnd,
}

impl Frame for ServerFrame {
    /// Attempt to parse this frame from a buffer
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Self, Error> {
        let kind = buf.try_get_u8()?;

        let frame = match kind {
            b'y' => {
                let paddle = match buf.try_get_u8()? {
                    b'L' => Paddle::Left,
                    b'R' => Paddle::Right,
                    _ => return Err(Error::Invalid),
                };

                ServerFrame::AcceptConnection(paddle)
            }

            b'n' => {
                // Parse rejection message
                let message = String::parse(buf)?;

                ServerFrame::RejectConnection(message)
            }

            b'S' => {
                let fixed = FixedGameState::parse(buf)?;
                let dynamic = DynamicGameState::parse(buf)?;

                ServerFrame::GameStart(InitialGameState { fixed, dynamic })
            }

            b'U' => ServerFrame::StateUpdate(DynamicGameState::parse(buf)?),

            b'P' => ServerFrame::GamePause,
            b'X' => ServerFrame::GameEnd,

            _ => return Err(Error::Invalid),
        };

        Ok(frame)
    }

    /// Write this frame to a stream
    async fn write<W>(&self, buf: &mut W) -> Result<(), std::io::Error>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            ServerFrame::AcceptConnection(paddle) => {
                buf.write_u8(b'y').await?;
                let paddle = match paddle {
                    Paddle::Left => b'L',
                    Paddle::Right => b'R',
                };
                buf.write_u8(paddle).await?;
            }
            ServerFrame::RejectConnection(message) => {
                buf.write_u8(b'n').await?;
                message.write(buf).await?;
            }
            ServerFrame::GameStart(initial_game_state) => {
                buf.write_u8(b'S').await?;
                initial_game_state.fixed.write(buf).await?;
                initial_game_state.dynamic.write(buf).await?;
            }
            ServerFrame::StateUpdate(dynamic_game_state) => {
                buf.write_u8(b'U').await?;
                dynamic_game_state.write(buf).await?;
            }
            ServerFrame::GameEnd => {
                buf.write_u8(b'X').await?;
            }
            Self::GamePause => {
                buf.write_u8(b'P').await?;
            }
        }

        Ok(())
    }
}
