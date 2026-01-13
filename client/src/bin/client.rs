use client::{game_state::GameState, listener::Listener, render::Renderer};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use protocol::{
    connection::Connection,
    frame::{ClientFrame, ServerFrame},
};
use tokio::{net::TcpStream, time};

const FPS: f32 = 15.;

#[tokio::main]
async fn main() {
    let conn = TcpStream::connect("127.0.0.1:12345").await.unwrap();
    let (mut writer, mut reader, _) = Connection::new(conn).into_parts();

    // Connection handshake
    writer.write_frame(&ClientFrame::Connect).await.unwrap();

    let paddle = match reader.read_frame::<ServerFrame>().await.unwrap() {
        ServerFrame::AcceptConnection(paddle) => paddle,
        ServerFrame::RejectConnection(message) => {
            panic!("Server rejected connection: {:?}", message)
        }
        frame => panic!("Unexpected server frame recieved: {:?}", frame),
    };

    writer
        .write_frame(&ClientFrame::ConnectedAck)
        .await
        .unwrap();

    // Wait for game to start
    let game_state = match reader.read_frame::<ServerFrame>().await.unwrap() {
        ServerFrame::GameStart(state) => state,
        frame => panic!("Unexpected server frame recieved: {:?}", frame),
    };
    let game_state = Arc::new(Mutex::new(GameState::from_initial_frame(game_state)));
    println!("Starting game");

    // Set up listener thread
    let (listener, receiver) = Listener::new(game_state.clone(), reader);
    tokio::spawn(listener.run());

    // Set up manager on new thread so renderer can run in main
    tokio::spawn(async move {
        tokio::select! {
            // Stop signal from server
            x = receiver => {}
            // Stop signal from client
            x = tokio::signal::ctrl_c() => {
                writer.write_frame(&ClientFrame::Disconnect).await.unwrap();
            }
            // Main client loop, shouldn't ever complete
            x = client_loop() => {}
        }
    });

    // Set up render in main thread
    let renderer = Renderer::new(game_state.clone(), FPS);
    renderer.run();
}

async fn client_loop() {
    loop {
        time::sleep(Duration::from_secs(1)).await;
    }
}
