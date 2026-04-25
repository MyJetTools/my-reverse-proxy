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

    let key_pair = KeyPair::generate().unwrap();

    let mut params = CertificateParams::new(subject_alt_names).unwrap();

    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(365 * 10);

    let cert = params.self_signed(&key_pair).unwrap();

    let cert_der = cert.der().clone();
    let key_pem = key_pair.serialize_pem();

    (cert_der, key_pem)
}

#[cfg(test)]
mod tests {

    #[test]
    fn generate_private_key() {
        let _ = super::generate("localhost".to_string());
    }
}
