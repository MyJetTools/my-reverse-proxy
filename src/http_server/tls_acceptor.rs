use std::sync::Arc;

use tokio_rustls::rustls::{
    version::{TLS12, TLS13},
    ServerConfig,
};

use crate::{
    app::AppContext,
    configurations::*,
    http_server::{client_cert_cell::ClientCertCell, server_cert_resolver::MyCertResolver},
};

use super::MyClientCertVerifier;

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
    let app_configuration = app.get_current_app_configuration().await;
    let certified_key = app_configuration
        .get_ssl_certified_key(endpoint_port, server_name)
        .await?;

    let endpoint_info = app_configuration.get_http_endpoint_info(endpoint_port, server_name)?;

    if let Some(client_cert_ca_id) = &endpoint_info.client_certificate_id {
        let client_cert_ca = app_configuration
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
            client_cert_ca_id.clone(),
            client_cert_cell.clone(),
            client_cert_ca.unwrap(),
            endpoint_port,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        //.with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        println!(
            "Applying ALPN protocols: {:?}",
            !endpoint_info.http_type.is_protocol_http1()
        );
        server_config.alpn_protocols =
            get_alpn_protocol(!endpoint_info.http_type.is_protocol_http1());
        return Ok((server_config, endpoint_info, Some(client_cert_cell)));
    }

    let mut server_config =
        tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

    server_config.alpn_protocols = get_alpn_protocol(!endpoint_info.http_type.is_protocol_http1());

    Ok((server_config, endpoint_info, None))
}

fn get_alpn_protocol(https2: bool) -> Vec<Vec<u8>> {
    if https2 {
        vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()]
    } else {
        vec![b"http/1.1".to_vec()]
    }
}
