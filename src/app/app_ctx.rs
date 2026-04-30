use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};

use super::*;
use encryption::aes::AesKey;
use my_ssh::SshAsyncChannel;
use my_tls::tokio_rustls::client::TlsStream;
use my_tls::tokio_rustls::rustls::sign::CertifiedKey;
use rust_extensions::{AppStates, UnsafeValue};
use tokio::{net::TcpStream, sync::Mutex};

use crate::{
    configurations::*,
    http2_client_pool::Http2ClientPool,
    http_client_connectors::*,
    http_client_pool::HttpClientPool,
    settings::ConnectionsSettingsModel,
    settings_compiled::SettingsCompiled,
    ssl::CertificatesCache,
    tcp_gateway::{client::TcpGatewayClient, server::TcpGatewayServer, TcpGatewayConnection},
    upstream_h1_pool::H1PoolRegistry,
    upstream_h2_pool::H2PoolRegistry,
};

use super::{ActiveListenPorts, CertPassKeys, Metrics, Prometheus};

pub const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

lazy_static::lazy_static! {
    pub static ref APP_CTX: AppContext = {
        let settings_model = SettingsCompiled::load_settings_block().unwrap();
        AppContext::new(settings_model)
    };
}

pub struct AppContext {
    pub http_over_ssh_clients_pool: HttpClientPool<SshAsyncChannel, HttpOverSshConnector>,

    pub http2_over_ssh_clients_pool: Http2ClientPool<SshAsyncChannel, HttpOverSshConnector>,

    pub h2_tcp_pools: H2PoolRegistry<TcpStream, HttpConnector>,
    pub h2_tls_pools: H2PoolRegistry<TlsStream<TcpStream>, HttpTlsConnector>,
    pub h2_uds_pools: H2PoolRegistry<tokio::net::UnixStream, UnixSocketHttpConnector>,

    pub h1_tcp_pools: H1PoolRegistry<TcpStream, HttpConnector>,
    pub h1_tls_pools: H1PoolRegistry<TlsStream<TcpStream>, HttpTlsConnector>,
    pub h1_uds_pools: H1PoolRegistry<tokio::net::UnixStream, UnixSocketHttpConnector>,

    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,
    pub default_h2_livness_url: Option<String>,

    pub token_secret_key: AesKey,
    pub current_configuration: AppConfiguration,
    pub states: Arc<AppStates>,

    pub show_error_description: UnsafeValue<bool>,
    pub prometheus: Arc<Prometheus>,
    pub metrics: Metrics,
    pub active_listen_ports: Mutex<ActiveListenPorts>,

    pub ssh_config_list: SshConfigList,

    pub allowed_users_list: AllowedUsersList,

    pub ssl_certificates_cache: CertificatesCache,

    pub ssh_cert_pass_keys: CertPassKeys,

    pub gateway_server: Option<TcpGatewayServer>,
    pub gateway_clients: HashMap<String, TcpGatewayClient>,
    pub http_control_port: Option<u16>,

    pub ssh_sessions_pool: SshSessionsPool,

    pub self_signed_cert: Arc<CertifiedKey>,
}

impl AppContext {
    pub fn new(settings_model: SettingsCompiled) -> Self {
        let http_control_port = settings_model.get_http_control_port();
        let connection_settings = settings_model.get_connections_settings();
        let default_h2_livness_url = settings_model.get_default_h2_livness_url();

        let token_secret_key = if let Some(session_key) = settings_model.get_session_key() {
            AesKey::new(get_token_secret_key_from_settings(session_key.as_bytes()).as_slice())
        } else {
            AesKey::new(generate_random_token_secret_key().as_slice())
        };

        let gateway_server =
            if let Some(gateway_server_settings) = settings_model.get_gateway_server() {
                let authorized_keys = gateway_server_settings.load_authorized_keys().unwrap();
                Some(TcpGatewayServer::new(
                    format!("0.0.0.0:{}", gateway_server_settings.port,),
                    authorized_keys,
                    false,
                    gateway_server_settings.is_debug(),
                ))
            } else {
                None
            };

        let mut gateway_clients = HashMap::new();

        let ssh_registry = &settings_model.ssh;
        for (id, client_settings) in settings_model.gateway_clients.iter() {
            let signing_key = client_settings
                .load_signing_key(id.as_str(), ssh_registry)
                .unwrap();
            let client = TcpGatewayClient::new(
                id.to_string(),
                client_settings.remote_host.to_string(),
                signing_key,
                client_settings.get_supported_compression(),
                client_settings.get_allow_incoming_forward_connections(),
                client_settings.get_connect_timeout(),
                client_settings.is_debug(),
                client_settings.get_sync_ssl_certificates(),
            );

            gateway_clients.insert(id.clone(), client);
        }

        Self {
            id: AtomicI64::new(0),
            connection_settings,
            default_h2_livness_url,
            // saved_client_certs: SavedClientCert::new(),
            token_secret_key,
            current_configuration: AppConfiguration::new(),
            states: Arc::new(AppStates::create_initialized()),
            prometheus: {
                let prom = Arc::new(Prometheus::new());
                my_http_client::set_task_metrics_hook(prom.clone());
                prom
            },
            ssl_certificates_cache: CertificatesCache::new(),
            //local_port_allocator: LocalPortAllocator::new(),
            //ssh_to_http_port_forward_pool: SshToHttpPortForwardPool::new(),
            show_error_description: UnsafeValue::new(
                settings_model.get_show_error_description_on_error_page(),
            ),
            metrics: Metrics::new(),
            active_listen_ports: Mutex::new(ActiveListenPorts::new()),
            ssh_config_list: SshConfigList::new(),
            allowed_users_list: AllowedUsersList::new(),
            ssh_cert_pass_keys: CertPassKeys::new(),
            http_over_ssh_clients_pool: HttpClientPool::new(),

            http2_over_ssh_clients_pool: Http2ClientPool::new(),
            h2_tcp_pools: H2PoolRegistry::new(),
            h2_tls_pools: H2PoolRegistry::new(),
            h2_uds_pools: H2PoolRegistry::new(),
            h1_tcp_pools: H1PoolRegistry::new(),
            h1_tls_pools: H1PoolRegistry::new(),
            h1_uds_pools: H1PoolRegistry::new(),
            gateway_server: gateway_server,
            gateway_clients: gateway_clients,
            http_control_port,
            ssh_sessions_pool: SshSessionsPool::new(),
            self_signed_cert: Arc::new(
                crate::self_signed_cert::generate(
                    crate::self_signed_cert::SELF_SIGNED_CERT_NAME.to_string(),
                )
                .unwrap(),
            ),
        }
    }

    pub fn get_next_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn get_gateway_by_id_with_next_connection_id(
        &self,
        gateway_id: &str,
    ) -> Option<(Arc<TcpGatewayConnection>, u32)> {
        if let Some(server_gateway) = self.gateway_server.as_ref() {
            if let Some(result) = server_gateway.get_gateway_connection(gateway_id) {
                return Some((result, server_gateway.get_next_connection_id()));
            }
        }

        let gateway_client = self.gateway_clients.get(gateway_id)?;

        let result = gateway_client.get_gateway_connection(gateway_id)?;

        Some((result, gateway_client.get_next_connection_id()))
    }

    pub fn get_gateway_by_id(&self, gateway_id: &str) -> Option<Arc<TcpGatewayConnection>> {
        if let Some(server_gateway) = self.gateway_server.as_ref() {
            if let Some(result) = server_gateway.get_gateway_connection(gateway_id) {
                return Some(result);
            }
        }

        let gateway_client = self.gateway_clients.get(gateway_id)?;

        gateway_client.get_gateway_connection(gateway_id)
    }
}

fn generate_random_token_secret_key() -> Vec<u8> {
    let mut result = Vec::with_capacity(48);

    let mut key = vec![];

    while result.len() < 48 {
        if key.len() == 0 {
            key = uuid::Uuid::new_v4().as_bytes().to_vec();
        }

        result.push(key.pop().unwrap());
    }

    result
}

fn get_token_secret_key_from_settings(session_key: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(48);

    let mut key = vec![];

    while result.len() < 48 {
        if key.len() == 0 {
            key = session_key.to_vec();
        }

        result.push(key.pop().unwrap());
    }

    result
}
