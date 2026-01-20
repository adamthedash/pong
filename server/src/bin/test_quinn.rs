use std::{net::SocketAddr, str::FromStr, sync::Arc};

use quinn::{
    ClientConfig, Endpoint, ServerConfig,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    },
};
use rcgen::CertifiedKey;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};

#[tokio::main]
async fn main() {
    let server_name = "bingbong";
    let address = SocketAddr::from_str("127.0.0.1:12345").unwrap();

    // Generate self-signed certificate
    let (cert, private_key) = rcgen::generate_simple_self_signed(&[server_name.to_string()])
        .map(|CertifiedKey { cert, signing_key }| {
            (
                CertificateDer::from(cert),
                PrivatePkcs8KeyDer::from(signing_key.serialize_der()),
            )
        })
        .unwrap();

    // Create server endpoint
    let server = {
        let config =
            ServerConfig::with_single_cert(vec![cert.clone()], private_key.into()).unwrap();

        Endpoint::server(config, address).unwrap()
    };

    // Start listening for a connection
    tokio::spawn({
        let server = server.clone();
        async move {
            // Wait for an incoming conection
            let incoming = server.accept().await.unwrap();
            // Accept the connection, giving a handle to the connection stream(?)
            let conn = incoming.await.unwrap();
            println!("Got connection: {:?}", conn.remote_address());

            let (mut send, mut recv) = conn.accept_bi().await.unwrap();
            println!("Accepted bi connection");

            let mut message = String::new();
            recv.read_to_string(&mut message).await.unwrap();
            println!("Got message: {:?}", message);

            send.write_all(b"pong").await.unwrap();
            send.finish().unwrap();
            println!("Waiting for client to receive data");
            send.stopped().await.unwrap();
            println!("Data received & SendStream closed");
        }
    });

    // Create the client endpoint
    let client = {
        let mut cert_store = rustls::RootCertStore::empty();
        cert_store.add(cert).unwrap();

        let config = ClientConfig::with_root_certificates(Arc::new(cert_store)).unwrap();

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
        endpoint.set_default_client_config(config);
        endpoint
    };

    // Connect to the server
    let connection = client.connect(address, server_name).unwrap().await.unwrap();
    println!("Connected to: {:?}", connection.remote_address());

    let (send, mut recv) = connection.open_bi().await.unwrap();
    let mut send = BufWriter::new(send);
    println!("Opened bi connection");

    send.write_all(b"ping").await.unwrap();
    send.write_u8(b'!').await.unwrap();
    send.flush().await.unwrap();
    send.into_inner().finish().unwrap();

    let mut message = String::new();
    recv.read_to_string(&mut message).await.unwrap();
    println!("Got response: {:?}", message);

    server.wait_idle().await;
    println!("Server shutting down");
}
