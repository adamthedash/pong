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
    env_logger::init();

    log::info!("Connecting to server");
    let (writer, mut reader, _paddle) = connect("127.0.0.1:12345").await.unwrap();
    let writer = Arc::new(TMutex::new(writer));
    log::info!("Connected to server");

    // Wait for game to start
    log::info!("Waiting for other player");
    let game_state = match reader.read_frame::<ServerFrame>().await.unwrap() {
        ServerFrame::GameStart(state) => state,
        frame => panic!("Unexpected server frame recieved: {:?}", frame),
    };
    let game_state = Arc::new(SMutex::new(GameState::from_initial_frame(game_state)));
    log::info!("Starting game");

    // Set up listener thread
    let listener = Listener::new(game_state.clone(), reader);

    // Set up controller thread
    let (controller, controller_sender) = Controller::new(writer.clone());

    // Set up render thread
    let (renderer, cancel_renderer) = Renderer::new(game_state.clone(), FPS, controller_sender);

    // Set up manager on new thread so renderer can run in main
    tokio::spawn(async move {
        tokio::select! {
            // Stop signal from server
            x = listener.run() => {
                match x {
                    Ok(()) => {
                        log::info!("Match ended");
                    }
                    Err(e) => {
                        log::error!("Listener error: {:?}", e);
                    }

                }
            }
            // Error during input handling
            x = controller.run() => {
                let e = x.expect_err("Controller cannot finish successfully");
                log::error!("Controller error: {:?}", e);
            }
            // Stop signal from client terminal
            _ = tokio::signal::ctrl_c() => {
                log::info!("Ctrl-C interrupt");
            }
        }

        // Send disconnect message to server no matter what
        if let Err(e) = writer
            .lock()
            .await
            .write_frame(&ClientFrame::Disconnect)
            .await
        {
            log::error!("Error sending disconnect to server: {:?}", e);
        }

        cancel_renderer.cancel();
    });

    // Render thread shut down
    match renderer.run() {
        Ok(()) => {
            log::info!("Application shut down");
        }
        Err(e) => {
            log::error!("Renderer error: {:?}", e);
        }
    }
}
