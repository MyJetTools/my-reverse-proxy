//! Standalone helper that generates an Ed25519 keypair in OpenSSH format,
//! ready to drop into the gateway settings.
//!
//! Usage:
//!     cargo run --bin gateway_keygen -- <out_path>
//!
//! Writes:
//!     <out_path>          — OpenSSH-format private key (referenced from
//!                            settings.ssh.<id>.private_key_file)
//!     <out_path>.pub      — OpenSSH-format public key (drop the path into
//!                            gateway_server.authorized_keys)

use ed25519_dalek::SigningKey;
use rand_core::{OsRng, RngCore};
use ssh_key::private::Ed25519Keypair;
use ssh_key::{LineEnding, PrivateKey};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(out_path) = args.next() else {
        eprintln!("Usage: gateway_keygen <out_path>");
        std::process::exit(2);
    };

    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let signing = SigningKey::from_bytes(&seed);
    let public_bytes = signing.verifying_key().to_bytes();
    let private_bytes = signing.to_bytes();

    let keypair = Ed25519Keypair {
        public: ssh_key::public::Ed25519PublicKey(public_bytes),
        private: ssh_key::private::Ed25519PrivateKey::from_bytes(&private_bytes),
    };

    let comment = format!(
        "gateway-{}",
        std::path::Path::new(&out_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("identity")
    );

    let private_key = PrivateKey::new(keypair.into(), comment.clone())
        .expect("Failed to construct OpenSSH private key");

    let private_pem = private_key
        .to_openssh(LineEnding::LF)
        .expect("Failed to encode OpenSSH private key");

    let public_openssh = private_key
        .public_key()
        .to_openssh()
        .expect("Failed to encode OpenSSH public key");

    let public_path = format!("{out_path}.pub");

    std::fs::write(&out_path, private_pem.as_bytes())
        .expect("Failed to write private key file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&out_path, perms)
            .expect("Failed to chmod private key file");
    }

    let public_line = format!("{public_openssh} {comment}\n");
    std::fs::write(&public_path, public_line.as_bytes())
        .expect("Failed to write public key file");

    println!("Wrote {out_path} (priv) and {public_path} (pub).");
    println!("Public key OpenSSH line:");
    println!("    {public_openssh} {comment}");
}
