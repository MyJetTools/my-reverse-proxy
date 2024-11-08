use rustls_pki_types::{CertificateDer, PrivateKeyDer};

pub fn load_certs(src: Vec<u8>) -> Vec<CertificateDer<'static>> {
    // Open certificate file.

    let mut reader = std::io::BufReader::new(src.as_slice());

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
pub fn load_private_key(src: Vec<u8>) -> Result<PrivateKeyDer<'static>, String> {
    let mut reader = std::io::BufReader::new(src.as_slice());

    let private_key = rustls_pemfile::private_key(&mut reader);

    if let Err(err) = &private_key {
        return Err(format!("Error loading private key: {:?}", err));
    }

    let private_key = private_key.unwrap();

    if private_key.is_none() {
        return Err(format!("No private key found in file"));
    }

    Ok(private_key.unwrap())

    //  Ok(private_key.into())
}
