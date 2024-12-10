use std::sync::Arc;

use my_tls::tokio_rustls;

use tokio_rustls::rustls::{
    version::{TLS12, TLS13},
    ServerConfig,
};

use super::*;
use crate::{app::AppContext, configurations::*};

use super::MyClientCertVerifier;

pub async fn create_config(
    app: Arc<AppContext>,
    configuration: Arc<HttpListenPortConfiguration>,
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
    let ssl_cert_result = configuration.get_ssl_certificate(server_name);

    if ssl_cert_result.is_none() {
        return Err(format!(
            "No ssl certificate found for server_name: {}",
            server_name
        ));
    }

    let (ssl_cert_id, http_endpoint_info) = ssl_cert_result.unwrap();

    let (ssl_cert_key, client_cert_ca) = if ssl_cert_id.as_str()
        == crate::self_signed_cert::SELF_SIGNED_CERT_NAME
    {
        let client_ca = app
            .ssl_certificates_cache
            .read(|app_config| {
                let client_cert_ca = if let Some(client_cert_ca_id) =
                    http_endpoint_info.client_certificate_id.as_ref()
                {
                    if let Some(client_cert_ca) = app_config.client_ca.get(client_cert_ca_id.into())
                    {
                        Some(client_cert_ca)
                    } else {
                        return Err(format!(
                            "Client certificate ca [{}] not found for endpoint: {}",
                            client_cert_ca_id.as_str(),
                            endpoint_port
                        ));
                    }
                } else {
                    None
                };

                Ok(client_cert_ca)
            })
            .await?;

        let certified_key = Arc::new(crate::self_signed_cert::generate(server_name.to_string())?);
        (certified_key, client_ca)
    } else {
        app.ssl_certificates_cache
            .read(|app_config| {
                let ssl_cert = app_config.ssl_certs.get(ssl_cert_id);

                if ssl_cert.is_none() {
                    return Err(format!(
                        "No ssl certificate found with id: {}",
                        ssl_cert_id.as_str()
                    ));
                }

                let ssl_cert_holder = ssl_cert.unwrap();

                let client_cert_ca = if let Some(client_cert_ca_id) =
                    http_endpoint_info.client_certificate_id.as_ref()
                {
                    if let Some(client_cert_ca) = app_config.client_ca.get(client_cert_ca_id.into())
                    {
                        if client_cert_ca.is_revoked() {
                            None
                        } else {
                            Some(client_cert_ca)
                        }
                    } else {
                        return Err(format!(
                            "Client certificate ca [{}] not found for endpoint: {}",
                            client_cert_ca_id.as_str(),
                            endpoint_port
                        ));
                    }
                } else {
                    None
                };

                Ok((ssl_cert_holder.ssl_cert.get_certified_key(), client_cert_ca))
            })
            .await?
    };

    if let Some(client_cert_ca) = client_cert_ca {
        let client_cert_cell = Arc::new(ClientCertCell::new());

        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            client_cert_cell.clone(),
            client_cert_ca,
            endpoint_port,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_cert_resolver(Arc::new(MyCertResolver::new(ssl_cert_key)));

        //.with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        server_config.alpn_protocols =
            get_alpn_protocol(!http_endpoint_info.listen_endpoint_type.is_http1());
        return Ok((server_config, http_endpoint_info, Some(client_cert_cell)));
    }

    let mut server_config =
        tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(MyCertResolver::new(ssl_cert_key)));

    server_config.alpn_protocols =
        get_alpn_protocol(!http_endpoint_info.listen_endpoint_type.is_http1());

    Ok((server_config, http_endpoint_info, None))
}

fn get_alpn_protocol(https2: bool) -> Vec<Vec<u8>> {
    if https2 {
        vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()]
    } else {
        vec![b"http/1.1".to_vec()]
    }
}
