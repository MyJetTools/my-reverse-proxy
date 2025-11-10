use std::{sync::Arc, time::Duration};

use crate::configurations::ListenConfiguration;

use super::ListenServerHandler;

pub fn start_listen_unix_server(host: Arc<String>) -> Arc<ListenServerHandler> {
    let listen_server_handler = Arc::new(ListenServerHandler::new());
    tokio::spawn(accept_unix_connections_loop(
        host,
        listen_server_handler.clone(),
    ));

    listen_server_handler
}

async fn accept_unix_connections_loop(
    listen_host: Arc<String>,
    listen_server_handler: Arc<ListenServerHandler>,
) {
    let removing_socket = tokio::fs::remove_file(listen_host.as_str());

    let result = tokio::time::timeout(Duration::from_secs(5), removing_socket).await;
    if result.is_err() {
        panic!("Can not remove old unix socket: {}. Timeout", listen_host);
    };

    let listener = match tokio::net::UnixListener::bind(listen_host.as_str()) {
        Ok(listener) => listener,
        Err(err) => {
            panic!(
                "Can not start listening server `{}`. Err: {:?}",
                listen_host.as_str(),
                err
            )
        }
    };

    while !crate::app::APP_CTX.states.is_shutting_down() {
        let accepted_connection_feature = listener.accept();

        let stop_endpoint_feature = listen_server_handler.await_stop();

        tokio::select! {
            accepted_connection = accepted_connection_feature => {
                if let Err(err) = &accepted_connection {
                        println!(
                            "Error accepting connection {}. Err: {:?}",
                            listen_host.as_str(), err
                        );

                    continue;
                }



                let (unix_stream, _) = accepted_connection.unwrap();





                handle_accepted_connection(unix_stream, listen_host.clone()).await;

            }
            _ = stop_endpoint_feature => {
                break;
            }
        }
    }

    if listen_server_handler.is_shutting_down() {
        listen_server_handler.set_tcp_thread_stopped();
    }
}

async fn handle_accepted_connection(
    mut accepted_connection: tokio::net::UnixStream,
    listen_host: Arc<String>,
) {
    let endpoint_type = crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            let listen_config = config
                .listen_unix_socket_endpoints
                .get(&listen_host)
                .cloned();

            listen_config
        })
        .await;

    if endpoint_type.is_none() {
        use tokio::io::AsyncWriteExt;
        let _ = accepted_connection.shutdown().await;
        return;
    }

    let endpoint_type = endpoint_type.unwrap();

    match endpoint_type {
        ListenConfiguration::Http(configuration) => match configuration.listen_endpoint_type {
            crate::configurations::ListenHttpEndpointType::Http1 => {
                crate::h1_proxy_server::kick_h1_unix_reverse_proxy_server_from_http(
                    accepted_connection,
                    configuration,
                );
                //super::http::handle_connection(accepted_connection, listening_addr, configuration)
                //    .await;
            }
            crate::configurations::ListenHttpEndpointType::Http2 => {
                super::http2::handle_connection(
                    accepted_connection.into(),
                    listen_host.clone().into(),
                    configuration,
                )
                .await;
            }
            crate::configurations::ListenHttpEndpointType::Https1 => {
                panic!(
                    "Tls as Https1 can not be applied to Unix socket. Host: {}",
                    listen_host.as_str()
                );
            }
            crate::configurations::ListenHttpEndpointType::Https2 => {
                panic!(
                    "Tls as Https2 can not be applied to Unix socket. Host: {}",
                    listen_host.as_str()
                );
            }
            crate::configurations::ListenHttpEndpointType::Mcp => {
                panic!(
                    "Mcp is not implemented for Unix socket. Host: {}",
                    listen_host.as_str()
                );
            }
        },

        ListenConfiguration::Tcp(configuration) => match configuration.remote_host.as_ref() {
            crate::configurations::MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                super::tcp_port_forward::tcp_over_gateway::handle_connection(
                    accepted_connection.into(),
                    configuration.clone(),
                    id,
                    remote_host.clone(),
                )
                .await;
            }
            crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh { .. } => {
                panic!(
                    "Unix socket over ssh is not supported. Host: '{}'",
                    listen_host.as_str()
                );
            }
            crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                super::tcp_port_forward::tcp::handle_connection(
                    accepted_connection.into(),
                    configuration.clone(),
                    remote_host.clone(),
                )
                .await;
            }
        },

        ListenConfiguration::Mpc(_) => {
            panic!(
                "Mcp can not be listen as a part of Unix socket. Host: {}",
                listen_host.as_str()
            );
        }
    }
}
