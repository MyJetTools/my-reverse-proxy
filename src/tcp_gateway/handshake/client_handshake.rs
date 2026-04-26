use std::sync::Arc;

use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use tokio::net::TcpStream;
use x25519_dalek::{EphemeralSecret, PublicKey};
use zeroize::Zeroize;

use encryption::aes::AesKey;

use super::protocol::{
    build_client_handshake, derive_session_key, read_handshake_frame, write_handshake_frame,
    ServerHandshakeFrame, PROTOCOL_VERSION,
};

pub async fn perform_client_handshake(
    stream: &mut TcpStream,
    signing_key: &SigningKey,
    gateway_name: &str,
) -> Result<Arc<AesKey>, String> {
    let eph_secret = EphemeralSecret::random_from_rng(OsRng);
    let eph_pub = PublicKey::from(&eph_secret);
    let eph_pub_bytes = eph_pub.to_bytes();

    let frame = build_client_handshake(eph_pub_bytes, signing_key, gateway_name);
    write_handshake_frame(stream, &frame.encode()).await?;

    let server_body = read_handshake_frame(stream).await?;
    let server = ServerHandshakeFrame::decode(&server_body)?;
    if server.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "ServerHandshake: unsupported protocol_version {}",
            server.protocol_version
        ));
    }

    let server_eph = PublicKey::from(server.server_eph_pub);
    let shared = eph_secret.diffie_hellman(&server_eph);
    let aes = derive_session_key(&eph_pub, &server_eph, &shared);

    let mut shared_bytes = shared.as_bytes().to_owned();
    shared_bytes.zeroize();

    Ok(Arc::new(aes))
}
