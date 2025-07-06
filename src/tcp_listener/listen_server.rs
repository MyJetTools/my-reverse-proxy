use std::{net::SocketAddr, sync::Arc};

use crate::{configurations::ListenConfiguration, tcp_or_unix::*};

use super::ListenServerHandler;

pub struct AcceptedTcpConnection {
    pub network_stream: MyNetworkStream,
    pub addr: SocketAddr,
}

pub fn start_listen_server(listening_addr: SocketAddr) -> Arc<ListenServerHandler> {
    let listen_server_handler = Arc::new(ListenServerHandler::new());
    tokio::spawn(accept_connections_loop(
        listening_addr,
        listen_server_handler.clone(),
    ));

    listen_server_handler
}

async fn accept_connections_loop(
    listening_addr: SocketAddr,
    listen_server_handler: Arc<ListenServerHandler>,
) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();

    while !crate::app::APP_CTX.states.is_shutting_down() {
        let accepted_connection_feature = listener.accept();

        let stop_endpoint_feature = listen_server_handler.await_stop();

        tokio::select! {
            accepted_connection = accepted_connection_feature => {
                if let Err(err) = &accepted_connection {
                        println!(
                            "Error accepting connection {}. Err: {:?}",
                            listening_addr, err
                        );

                    continue;
                }



                let (tcp_stream, addr) = accepted_connection.unwrap();

                let accepted_connection = AcceptedTcpConnection{
                    network_stream : MyNetworkStream::Tcp(tcp_stream),
                    addr
                };

                handle_accepted_connection(accepted_connection, listening_addr).await;

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
    mut accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
) {
    let listen_port = listening_addr.port();

    let endpoint_type = crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            let listen_config = config.listen_endpoints.get(&listen_port).cloned();

            if let Some(listen_config) = &listen_config {
                if let Some(white_list_id) = listen_config.get_white_list_id() {
                    if !config
                        .white_list_ip_list
                        .is_white_listed(white_list_id, &listening_addr.ip())
                    {
                        return None;
                    }
                }
            }

            listen_config
        })
        .await;

    if endpoint_type.is_none() {
        let _ = accepted_connection.network_stream.shutdown().await;
        return;
    }

    let endpoint_type = endpoint_type.unwrap();

    match endpoint_type {
        ListenConfiguration::Http(configuration) => match configuration.listen_endpoint_type {
            crate::configurations::ListenHttpEndpointType::Http1 => {
                super::http::handle_connection(accepted_connection, listening_addr, configuration)
                    .await;
            }
            crate::configurations::ListenHttpEndpointType::Http2 => {
                super::http2::handle_connection(accepted_connection, listening_addr, configuration)
                    .await;
            }
            crate::configurations::ListenHttpEndpointType::Https1 => {
                super::https::handle_connection(accepted_connection, listening_addr, configuration)
                    .await;
            }
            crate::configurations::ListenHttpEndpointType::Https2 => {
                super::https::handle_connection(accepted_connection, listening_addr, configuration)
                    .await;
            }
        },

        ListenConfiguration::Tcp(configuration) => match configuration.remote_host.as_ref() {
            crate::configurations::MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                super::tcp_port_forward::tcp_over_gateway::handle_connection(
                    accepted_connection,
                    listening_addr,
                    configuration.clone(),
                    id.clone(),
                    remote_host.clone(),
                )
                .await;
            }
            crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh {
                ssh_credentials,
                remote_host,
            } => {
                super::tcp_port_forward::tcp_over_ssh::handle_connection(
                    accepted_connection,
                    listening_addr,
                    configuration.clone(),
                    ssh_credentials.clone(),
                    remote_host.clone(),
                )
                .await;
            }
            crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                super::tcp_port_forward::tcp::handle_connection(
                    accepted_connection,
                    listening_addr,
                    configuration.clone(),
                    remote_host.clone(),
                )
                .await;
            }
        },
    }
}
