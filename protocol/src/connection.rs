use std::{io::Cursor, net::SocketAddr};

use crate::frame::{self, Frame};
use bytes::{Buf, BytesMut};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};

#[derive(Debug)]
pub enum Error {
    Frame(frame::Error),
    IO(std::io::Error),
    ConnectionClosed,
    ConnectionInterrupted,
}

pub struct Reader {
    connection: OwnedReadHalf,
    buffer: BytesMut,
}

impl Reader {
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
    connection: OwnedWriteHalf,
}

impl Writer {
    pub async fn write_frame<F: Frame>(&mut self, frame: &F) -> Result<(), Error> {
        frame.write(&mut self.connection).await.map_err(Error::IO)
    }

    pub async fn shutdown(mut self) -> Result<(), std::io::Error> {
        self.connection.shutdown().await
    }
}

/// Layer to handle communication over the wire between server & client
pub struct Connection {
    reader: Reader,
    writer: Writer,
    address: SocketAddr,
}

impl Connection {
    pub fn new(connection: TcpStream) -> Self {
        let address = connection
            .peer_addr()
            .expect("Failed to get peer connection address");
        let (reader, writer) = connection.into_split();

        Self {
            address,
            reader: Reader {
                connection: reader,
                buffer: BytesMut::with_capacity(1024),
            },
            writer: Writer { connection: writer },
        }
    }

    pub async fn read_frame<F: Frame>(&mut self) -> Result<F, Error> {
        self.reader.read_frame().await
    }

    pub async fn write_frame<F: Frame>(&mut self, frame: &F) -> Result<(), Error> {
        self.writer.write_frame(frame).await
    }

    pub async fn shutdown(self) -> Result<(), std::io::Error> {
        self.writer.shutdown().await
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn into_parts(self) -> (Writer, Reader, SocketAddr) {
        (self.writer, self.reader, self.address)
    }
}

impl PartialEq for Connection {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl PartialEq<SocketAddr> for Connection {
    fn eq(&self, other: &SocketAddr) -> bool {
        self.address == *other
    }
}
