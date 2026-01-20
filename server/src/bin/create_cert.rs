use std::io::Write;

use quinn::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use rcgen::CertifiedKey;

fn main() {
    let server_name = "pingpong";

    // Generate self-signed certificate
    let (cert, private_key) = rcgen::generate_simple_self_signed(&[server_name.to_string()])
        .map(|CertifiedKey { cert, signing_key }| {
            (
                CertificateDer::from(cert),
                PrivatePkcs8KeyDer::from(signing_key.serialize_der()),
            )
        })
        .unwrap();

    // Save to disk

    let mut cert_file = std::fs::File::create("cert/cert.der").unwrap();
    cert_file.write_all(&cert).unwrap();
    let mut key_file = std::fs::File::create("cert/key.der").unwrap();
    key_file.write_all(private_key.secret_pkcs8_der()).unwrap();
}
