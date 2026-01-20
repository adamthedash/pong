use std::net::SocketAddr;

use quinn::{
    Endpoint, ServerConfig,
    rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};
use server::orchestration::GameServer;

const BROADCAST_RATE: f32 = 30.;
const TICK_RATE: f32 = 30.;

fn setup_server(address: SocketAddr) -> Endpoint {
    // Load pre-generated cert
    let cert = CertificateDer::from(std::fs::read("cert/cert.der").unwrap());
    let private_key = PrivatePkcs8KeyDer::from(std::fs::read("cert/key.der").unwrap());

    // Create server endpoint
    let config = ServerConfig::with_single_cert(vec![cert.clone()], private_key.into()).unwrap();

    Endpoint::server(config, address).unwrap()
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let endpoint = setup_server("127.0.0.1:12345".parse().unwrap());

    let mut game_server = GameServer::new(BROADCAST_RATE, TICK_RATE);

    loop {
        let conn = endpoint.accept().await.unwrap().await.unwrap();
        log::info!("Got connection from {:?}", conn.remote_address());

        game_server.connect_player(conn).await.unwrap();

        if game_server.ready() {
            log::info!("Game ready, spawing new thread");
            // There are enough players, spin off the game server to execute in a new thread and
            // start a new one for the next player
            tokio::spawn(async move {
                let mut game_server = game_server;
                game_server.run().await
            });
            game_server = GameServer::new(BROADCAST_RATE, TICK_RATE);
        }
    }
}
