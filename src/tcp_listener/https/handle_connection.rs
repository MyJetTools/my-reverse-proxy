use std::{net::SocketAddr, sync::Arc};

use hyper_util::rt::TokioIo;
use tokio::io::AsyncWriteExt;

use crate::configurations::*;
use crate::http_proxy_pass::{HttpListenPortInfo, HttpProxyPass};
use crate::tcp_listener::http_request_handler::https::HttpsRequestsHandler;
use crate::tcp_listener::AcceptedTcpConnection;

use super::ClientCertificateData;

pub async fn handle_connection(
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<HttpListenPortConfiguration>,
    connection_id: u64,
) {
    let endpoint_port = listening_addr.port();
    let result = super::utils::lazy_accept_tcp_stream(
        endpoint_port,
        accepted_connection.network_stream,
        configuration.clone(),
    )
    .await;

    let Ok(result) = result else {
        return;
    };

    let (mut tls_stream, endpoint_info, cn_user_name) = result;

    if let Some(ip_list_id) = endpoint_info.whitelisted_ip_list_id.as_ref() {
        let is_whitelisted = crate::app::APP_CTX
            .current_configuration
            .get(|config| {
                config
                    .white_list_ip_list
                    .is_white_listed(ip_list_id, &listening_addr.ip())
            })
            .await;

        if !is_whitelisted {
            let _ = tls_stream.shutdown().await;
            return;
        }
    }

    match endpoint_info.listen_endpoint_type {
        ListenHttpEndpointType::Http1 => {
            crate::h1_server::kick_h1_reverse_proxy_server(
                listening_addr,
                accepted_connection.addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
                configuration,
            );
        }
        ListenHttpEndpointType::Http2 => {
            kick_off_https2(
                listening_addr,
                accepted_connection.addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
                endpoint_port,
            )
            .await;
        }
        ListenHttpEndpointType::Https1 => {
            crate::h1_server::kick_h1_reverse_proxy_server(
                listening_addr,
                accepted_connection.addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
                configuration.clone(),
            );
        }
        ListenHttpEndpointType::Https2 => {
            kick_off_https2(
                listening_addr,
                accepted_connection.addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
                endpoint_port,
            )
            .await;
        }
        ListenHttpEndpointType::Mcp => {
            println!("New mcp connection {}", connection_id);
            super::super::mcp::run_mcp_connection(tls_stream, &endpoint_info, connection_id).await;
        }
    }
}

async fn kick_off_https2(
    listening_addr: SocketAddr,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    tls_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    client_certificate: Option<Arc<ClientCertificateData>>,
    endpoint_port: u16,
) {
    use hyper::service::service_fn;
    use hyper_util::server::conn::auto::Builder;

    use hyper_util::rt::TokioExecutor;

    let endpoint_name = format!("https://{}", listening_addr);

    crate::app::APP_CTX
        .prometheus
        .inc_http2_server_connections(endpoint_name.as_str());

    crate::app::APP_CTX
        .metrics
        .update(|itm| itm.connection_by_port.inc(&endpoint_port))
        .await;

    tokio::spawn(async move {
        let http_builder = Builder::new(TokioExecutor::new());

        let listening_port_info = HttpListenPortInfo {
            endpoint_type: endpoint_info.listen_endpoint_type,
            socket_addr,
        };

        let http_proxy_pass = HttpProxyPass::new(
            endpoint_info.clone(),
            listening_port_info,
            client_certificate,
        )
        .await;

        let https_requests_handler = HttpsRequestsHandler::new(http_proxy_pass, socket_addr);

        let https_requests_handler = Arc::new(https_requests_handler);

        let https_requests_handler_dispose = https_requests_handler.clone();

        let _ = http_builder
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::super::http_request_handler::https::handle_request(
                        https_requests_handler.clone(),
                        req,
                    )
                }),
            )
            .await;

        crate::app::APP_CTX
            .prometheus
            .dec_http2_server_connections(endpoint_name.as_str());

        crate::app::APP_CTX
            .metrics
            .update(|itm| itm.connection_by_port.dec(&endpoint_port))
            .await;

        println!("Http2 connection is gone {}", socket_addr);

        https_requests_handler_dispose.dispose().await;
    });
}
