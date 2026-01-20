use client::{control::Controller, game_state::GameState, listener::Listener, render::Renderer};
use quinn::{
    ClientConfig, Endpoint,
    rustls::{self, pki_types::CertificateDer},
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex as SMutex},
};

use protocol::{
    connection::{Reader, Writer},
    frame::{ClientFrame, Paddle, ServerFrame},
};
use tokio::sync::Mutex as TMutex;

const FPS: f32 = 15.;

fn setup_client() -> Endpoint {
    let mut cert_store = rustls::RootCertStore::empty();
    let cert = CertificateDer::from(std::fs::read("cert/cert.der").unwrap());
    cert_store.add(cert).unwrap();

    let config = ClientConfig::with_root_certificates(Arc::new(cert_store)).unwrap();

    let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(config);
    endpoint
}

/// Connect the client to server
async fn connect(
    client: Endpoint,
    address: SocketAddr,
) -> Result<(Writer, Reader, Paddle), protocol::connection::Error> {
    let connection = client.connect(address, "pingpong").unwrap().await.unwrap();

    let (send, recv) = connection.open_bi().await.unwrap();
    let mut writer = Writer::new(send);
    let mut reader = Reader::new(recv);

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
    let client = setup_client();
    let (writer, mut reader, _paddle) = connect(client, "127.0.0.1:12345".parse().unwrap())
        .await
        .unwrap();
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
