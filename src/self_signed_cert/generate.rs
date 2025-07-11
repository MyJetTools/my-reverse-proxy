use my_tls::tokio_rustls;
use rustls_pki_types::CertificateDer;

pub fn generate(cn_name: String) -> Result<tokio_rustls::rustls::sign::CertifiedKey, String> {
    let (cert, key_pair) = generate_pk(cn_name);

    let private_key = crate::ssl::certificates::load_private_key(key_pair.as_bytes().to_vec())?;

    Ok(crate::ssl::calc_cert_key(&private_key, vec![cert]))
}

fn generate_pk(cn_name: String) -> (CertificateDer<'static>, String) {
    use rcgen::*;

    let subject_alt_names = vec![cn_name];

    let certified_key = generate_simple_self_signed(subject_alt_names).unwrap();

    let cert = certified_key.cert.der().clone();

    let key_pair = certified_key.signing_key.serialize_pem();

    (cert, key_pair)
}

#[cfg(test)]
mod tests {

    #[test]
    fn generate_private_key() {
        let _ = super::generate("localhost".to_string());
    }
}
