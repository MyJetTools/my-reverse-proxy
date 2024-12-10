use std::{net::SocketAddr, sync::Arc};

use tokio::io::AsyncWriteExt;

use crate::{app::AppContext, configurations::ListenConfiguration};

use super::ListenServerHandler;

pub struct AcceptedTcpConnection {
    pub tcp_stream: tokio::net::TcpStream,
    pub addr: SocketAddr,
}

pub fn start_listen_server(
    listening_addr: SocketAddr,
    app: Arc<AppContext>,
) -> Arc<ListenServerHandler> {
    let listen_server_handler = Arc::new(ListenServerHandler::new());
    tokio::spawn(accept_connections_loop(
        listening_addr,
        app,
        listen_server_handler.clone(),
    ));

    listen_server_handler
}

async fn accept_connections_loop(
    listening_addr: SocketAddr,
    app: Arc<AppContext>,
    listen_server_handler: Arc<ListenServerHandler>,
) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();

    while !app.states.is_shutting_down() {
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
                    tcp_stream,
                    addr
                };

                handle_accepted_connection(app.clone(), accepted_connection, listening_addr).await;

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
    app: Arc<AppContext>,
    mut accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
) {
    let listen_port = listening_addr.port();

    let endpoint_type = app
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
        let _ = accepted_connection.tcp_stream.shutdown().await;
        return;
    }

    let endpoint_type = endpoint_type.unwrap();

    match endpoint_type {
        ListenConfiguration::Http(configuration) => match configuration.listen_endpoint_type {
            crate::configurations::ListenHttpEndpointType::Http1 => {
                super::http::handle_connection(
                    app,
                    accepted_connection,
                    listening_addr,
                    configuration,
                )
                .await;
            }
            crate::configurations::ListenHttpEndpointType::Http2 => {
                super::http2::handle_connection(
                    app,
                    accepted_connection,
                    listening_addr,
                    configuration,
                )
                .await;
            }
            crate::configurations::ListenHttpEndpointType::Https1 => {
                super::https::handle_connection(
                    app,
                    accepted_connection,
                    listening_addr,
                    configuration,
                )
                .await;
            }
            crate::configurations::ListenHttpEndpointType::Https2 => {
                super::https::handle_connection(
                    app,
                    accepted_connection,
                    listening_addr,
                    configuration,
                )
                .await;
            }
        },

        ListenConfiguration::Tcp(configuration) => {
            if configuration.remote_host.ssh_credentials.is_none() {
                super::tcp_port_forward::tcp::handle_connection(
                    app,
                    accepted_connection,
                    listening_addr,
                    configuration,
                )
                .await;
            } else {
                super::tcp_port_forward::tcp_over_ssh::handle_connection(
                    app,
                    accepted_connection,
                    listening_addr,
                    configuration,
                )
                .await;
            }
        }
    }
}
