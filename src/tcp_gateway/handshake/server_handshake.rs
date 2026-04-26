use std::sync::Arc;

use ed25519_dalek::VerifyingKey;
use rand_core::OsRng;
use tokio::net::TcpStream;
use x25519_dalek::{EphemeralSecret, PublicKey};
use zeroize::Zeroize;

use encryption::aes::AesKey;

use super::protocol::{
    derive_session_key, read_handshake_frame, validate_timestamp, write_handshake_frame,
    ClientHandshakeFrame, ServerHandshakeFrame, PROTOCOL_VERSION,
};

pub struct ServerHandshakeOutcome {
    pub session_key: Arc<AesKey>,
    pub gateway_name: String,
    pub timestamp_us: i64,
}

pub async fn perform_server_handshake(
    stream: &mut TcpStream,
    authorized_keys: &[VerifyingKey],
) -> Result<ServerHandshakeOutcome, String> {
    let body = read_handshake_frame(stream).await?;
    let client = ClientHandshakeFrame::decode(&body)?;

    if client.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "ClientHandshake: unsupported protocol_version {}",
            client.protocol_version
        ));
    }

    validate_timestamp(client.timestamp_us)?;

    let verifying_key = VerifyingKey::from_bytes(&client.client_id_pub)
        .map_err(|err| format!("ClientHandshake: invalid Ed25519 public key bytes: {err}"))?;

    let authorized = authorized_keys
        .iter()
        .any(|k| k.to_bytes() == verifying_key.to_bytes());
    if !authorized {
        return Err("ClientHandshake: client_id_pub is not in authorized_keys".to_string());
    }

    client.verify_signature(&verifying_key)?;

    let server_secret = EphemeralSecret::random_from_rng(OsRng);
    let server_pub = PublicKey::from(&server_secret);

    let server_frame = ServerHandshakeFrame::new(server_pub.to_bytes());
    write_handshake_frame(stream, &server_frame.encode()).await?;

    let client_pub = PublicKey::from(client.client_eph_pub);
    let shared = server_secret.diffie_hellman(&client_pub);
    let aes = derive_session_key(&client_pub, &server_pub, &shared);

    let mut shared_bytes = shared.as_bytes().to_owned();
    shared_bytes.zeroize();

    Ok(ServerHandshakeOutcome {
        session_key: Arc::new(aes),
        gateway_name: client.gateway_name,
        timestamp_us: client.timestamp_us,
    })
}
