use std::io::Cursor;

use crate::frame::{self, Frame};
use bytes::{Buf, BytesMut};
use quinn::{RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
pub enum Error {
    Frame(frame::Error),
    IO(std::io::Error),
    ConnectionClosed,
    ConnectionInterrupted,
}

pub struct Reader {
    connection: RecvStream,
    buffer: BytesMut,
}

impl Reader {
    pub fn new(stream: RecvStream) -> Self {
        Self {
            connection: stream,
            buffer: BytesMut::with_capacity(1024),
        }
    }

    /// Try to read a frame from this connection. Blocks until a frame or error is recieved
    pub async fn read_frame<F: Frame>(&mut self) -> Result<F, Error> {
        loop {
            let mut buf = Cursor::new(&self.buffer[..]);

            match Frame::parse(&mut buf) {
                Ok(frame) => {
                    // Successfully read a frame, so advance the real buffer
                    self.buffer.advance(buf.position() as usize);
                    return Ok(frame);
                }

                Err(e) => match e {
                    frame::Error::Incomplete => {
                        // Not enough in the buffer, try to read more from the stream
                        let bytes_read = self
                            .connection
                            .read_buf(&mut self.buffer)
                            .await
                            .map_err(Error::IO)?;

                        // 0 bytes read == end of stream
                        if bytes_read == 0 {
                            if self.buffer.is_empty() {
                                // Closed stream at end of a frame,
                                return Err(Error::ConnectionClosed);
                            } else {
                                // Closed connection mid-way through a frame
                                return Err(Error::ConnectionInterrupted);
                            }
                        }
                    }

                    // Some other issue while parsing the frame, bubble up
                    _ => return Err(Error::Frame(e)),
                },
            };
        }
    }
}

pub struct Writer {
    connection: SendStream,
}

impl Writer {
    pub fn new(stream: SendStream) -> Self {
        Self { connection: stream }
    }

    pub async fn write_frame<F: Frame>(&mut self, frame: &F) -> Result<(), Error> {
        frame.write(&mut self.connection).await.map_err(Error::IO)
    }

    pub async fn shutdown(mut self) -> Result<(), std::io::Error> {
        self.connection.shutdown().await
    }
}
