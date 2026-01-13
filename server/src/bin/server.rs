use protocol::connection::Connection;

use server::orchestration::GameServer;
use tokio::net::TcpListener;

const BROADCAST_RATE: f32 = 30.;
const TICK_RATE: f32 = 30.;

#[tokio::main]
async fn main() {
    env_logger::init();

    let listener = TcpListener::bind("127.0.0.1:12345").await.unwrap();

    let mut game_server = GameServer::new(BROADCAST_RATE, TICK_RATE);

    loop {
        let (socket, source) = listener.accept().await.unwrap();
        log::info!("Got connection: {:?} from {:?}", socket, source);

        game_server
            .connect_player(Connection::new(socket))
            .await
            .unwrap();

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
