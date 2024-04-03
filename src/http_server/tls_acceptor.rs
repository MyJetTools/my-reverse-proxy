use std::sync::Arc;

use tokio_rustls::rustls::{
    version::{TLS12, TLS13},
    ServerConfig,
};

use crate::{
    app::AppContext, app_configuration::HttpEndpointInfo,
    http_server::client_cert_cell::ClientCertCell,
};

use super::{server_cert_resolver::MyCertResolver, MyClientCertVerifier};

pub async fn create_config(
    app: Arc<AppContext>,
    server_name: &str,
    endpoint_port: u16,
) -> Result<
    (
        ServerConfig,
        Arc<HttpEndpointInfo>,
        Option<Arc<ClientCertCell>>,
    ),
    String,
> {
    let certified_key = app
        .current_app_configuration
        .read()
        .await
        .as_ref()
        .unwrap()
        .get_ssl_certified_key(endpoint_port)
        .await?;

    let endpoint_info = app
        .current_app_configuration
        .read()
        .await
        .as_ref()
        .unwrap()
        .get_http_endpoint_info(endpoint_port, server_name)
        .await?;
    if let Some(client_cert_ca_id) = &endpoint_info.client_certificate_id {
        let client_cert_ca = app
            .current_app_configuration
            .read()
            .await
            .as_ref()
            .unwrap()
            .client_certificates_cache
            .get(client_cert_ca_id);

        if client_cert_ca.is_none() {
            return Err(format!(
                "Client certificate ca not found: {} for endpoint: {}",
                client_cert_ca_id.as_str(),
                endpoint_port
            ));
        }

        let client_cert_cell = Arc::new(ClientCertCell::new());

        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            client_cert_cell.clone(),
            client_cert_ca.unwrap(),
            endpoint_port,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        println!(
            "Applying ALPN protocols: {:?}",
            !endpoint_info.http_type.is_http1()
        );
        server_config.alpn_protocols = get_alpn_protocol(!endpoint_info.http_type.is_http1());
        return Ok((server_config, endpoint_info, Some(client_cert_cell)));
    }

    let mut server_config =
        tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

    server_config.alpn_protocols = get_alpn_protocol(!endpoint_info.http_type.is_http1());

    Ok((server_config, endpoint_info, None))
}

fn get_alpn_protocol(https2: bool) -> Vec<Vec<u8>> {
    if https2 {
        vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()]
    } else {
        vec![b"http/1.1".to_vec()]
    }
}

/*

pub fn create_tls_acceptor(
    app: Arc<AppContext>,
    client_cert_ca: Option<Arc<ClientCertificateCa>>,
    endpoint_port: u16,
    connection_id: u64,
    certified_key: Arc<CertifiedKey>,
    https2: bool,
) -> TlsAcceptor {
    if let Some(client_cert_ca) = client_cert_ca {
        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            app.clone(),
            client_cert_ca,
            endpoint_port,
            connection_id,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        println!("Applying ALPN protocols: {:?}", https2);
        server_config.alpn_protocols = get_alpn_protocol(https2);

        return TlsAcceptor::from(Arc::new(server_config));
    }

    let mut server_config =
        tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

    server_config.alpn_protocols = get_alpn_protocol(https2);

    TlsAcceptor::from(Arc::new(server_config))
}
*/