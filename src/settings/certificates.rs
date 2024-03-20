use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use super::FileName;

pub fn load_certs(file_name: &FileName) -> Vec<CertificateDer<'static>> {
    // Open certificate file.

    let file_name = file_name.get_value();

    let cert_file = std::fs::File::open(file_name.as_str());

    if let Err(err) = &cert_file {
        panic!("Failed to open file {}. Err: {:?}", file_name.as_str(), err);
    }

    let cert_file = cert_file.unwrap();
    let mut reader = std::io::BufReader::new(cert_file);

    let certs = rustls_pemfile::certs(&mut reader);

    // Load and return certificate.
    let mut result = Vec::new();

    for cert in certs {
        let cert: rustls_pki_types::CertificateDer<'_> = cert.unwrap();
        result.push(cert);
    }

    result
}

// Load private key from file.
pub fn load_private_key(file_name: &FileName) -> PrivateKeyDer<'static> {
    let file_name = file_name.get_value();

    let key_file = std::fs::File::open(file_name.as_str());

    if let Err(err) = &key_file {
        panic!("Failed to open file {}. Err: {:?}", file_name.as_str(), err);
    }

    let key_file = key_file.unwrap();

    let mut reader = std::io::BufReader::new(key_file);

    let private_key = rustls_pemfile::private_key(&mut reader).unwrap();

    if private_key.is_none() {
        panic!("No private key found in file {}", file_name.as_str());
    }

    private_key.unwrap()

    //  Ok(private_key.into())
}
