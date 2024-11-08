use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use my_tls::tokio_rustls;
use tokio::sync::Mutex;
use tokio_rustls::rustls::sign::CertifiedKey;

use crate::{crl::ListOfCrl, ssl::*};

use super::*;

pub const SELF_SIGNED_CERT_NAME: &str = "self_signed";

pub struct AppConfiguration {
    pub client_certificates_cache: ClientCertificatesCache,
    pub ssl_certificates_cache: SslCertificatesCache,
    pub http_endpoints: BTreeMap<u16, HttpListenPortConfiguration>,
    pub tcp_endpoints: BTreeMap<u16, Arc<TcpEndpointHostConfig>>,
    pub tcp_over_ssh_endpoints: BTreeMap<u16, Arc<TcpOverSshEndpointHostConfig>>,
    pub crl: HashMap<String, FileSource>,
    pub list_of_crl: Mutex<ListOfCrl>,
}

impl AppConfiguration {
    pub async fn get_ssl_certified_key(
        &self,
        listen_port: u16,
        server_name: &str,
    ) -> Result<Arc<CertifiedKey>, String> {
        if let Some(port_configuration) = self.http_endpoints.get(&listen_port) {
            let ssl_certificate_id = port_configuration.get_ssl_certificate(server_name);

            if ssl_certificate_id.is_none() {
                return Err(format!(
                    "No matching configuration for server_name {} on port {}.",
                    server_name, listen_port
                ));
            }

            let ssl_certificate_id = ssl_certificate_id.unwrap();

            if ssl_certificate_id.as_str() == SELF_SIGNED_CERT_NAME {
                return Ok(Arc::new(crate::self_signed_cert::generate(
                    server_name.to_string(),
                )?));
            }

            if let Some(key) = self
                .ssl_certificates_cache
                .get_certified_key(&ssl_certificate_id)
                .await
            {
                return Ok(key);
            } else {
                return Err(format!(
                    "Can not find ssl_certified_key for port: {}",
                    listen_port
                ));
            }
        } else {
            Err(format!(
                "Can not find ssl_certified_key for port: {}",
                listen_port
            ))
        }
    }

    pub fn get_http_endpoint_info(
        &self,
        listen_port: u16,
        server_name: &str,
    ) -> Result<Arc<HttpEndpointInfo>, String> {
        if let Some(listen_port_config) = self.http_endpoints.get(&listen_port) {
            for endpoint_info in &listen_port_config.endpoint_info {
                if endpoint_info.is_my_endpoint(server_name) {
                    return Ok(endpoint_info.clone());
                }
            }
        }

        Err(format!("Not port is listening at port: {}", listen_port))
    }
}
