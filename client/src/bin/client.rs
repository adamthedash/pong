use client::{control::Controller, game_state::GameState, listener::Listener, render::Renderer};
use std::sync::{Arc, Mutex as SMutex};

use protocol::{
    connection::{Connection, Reader, Writer},
    frame::{ClientFrame, Paddle, ServerFrame},
};
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    sync::Mutex as TMutex,
};

const FPS: f32 = 15.;

/// Connect the client to server
async fn connect(
    address: impl ToSocketAddrs,
) -> Result<(Writer, Reader, Paddle), protocol::connection::Error> {
    let conn = TcpStream::connect(address)
        .await
        .map_err(protocol::connection::Error::IO)?;
    let (mut writer, mut reader, _) = Connection::new(conn).into_parts();

    // Connection handshake
    writer.write_frame(&ClientFrame::Connect).await.unwrap();

    let paddle = match reader.read_frame::<ServerFrame>().await? {
        ServerFrame::AcceptConnection(paddle) => paddle,
        ServerFrame::RejectConnection(message) => {
            panic!("Server rejected connection: {:?}", message)
        }
        frame => panic!("Unexpected server frame recieved: {:?}", frame),
    };

    writer.write_frame(&ClientFrame::ConnectedAck).await?;

    Ok((writer, reader, paddle))
}

#[tokio::main]
async fn main() {
    let (writer, mut reader, _paddle) = connect("127.0.0.1:12345").await.unwrap();
    let writer = Arc::new(TMutex::new(writer));

    // Wait for game to start
    let game_state = match reader.read_frame::<ServerFrame>().await.unwrap() {
        ServerFrame::GameStart(state) => state,
        frame => panic!("Unexpected server frame recieved: {:?}", frame),
    };
    let game_state = Arc::new(SMutex::new(GameState::from_initial_frame(game_state)));
    println!("Starting game");

    // Set up listener thread
    // TODO: Account for errors in ::run() for non-main threads
    let (listener, receiver) = Listener::new(game_state.clone(), reader);
    tokio::spawn(listener.run());

    // Set up controller thread
    let (controller, controller_sender) = Controller::new(writer.clone());
    tokio::spawn(controller.run());

    // Set up manager on new thread so renderer can run in main
    tokio::spawn(async move {
        tokio::select! {
            // Stop signal from server
            _ = receiver => {}
            // Stop signal from client terminal
            _ = tokio::signal::ctrl_c() => {
                writer.lock().await.write_frame(&ClientFrame::Disconnect).await.unwrap();
            }
            // TODO: Stop signal from client GUI (user presses X)
            // TODO: Error from any thread
        }
    });

    // Set up render in main thread
    let renderer = Renderer::new(game_state.clone(), FPS, controller_sender);
    renderer.run();
}
