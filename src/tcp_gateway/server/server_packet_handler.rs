use std::sync::Arc;

use rust_extensions::{date_time::DateTimeAsMicroseconds, SliceOrVec};

use crate::tcp_gateway::*;

pub struct TcpGatewayServerPacketHandler;

#[async_trait::async_trait]
impl TcpGatewayPacketHandler for TcpGatewayServerPacketHandler {
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        tcp_gateway: &Arc<TcpGatewayInner>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) -> Result<(), String> {
        match contract {
            TcpGatewayContract::Handshake {
                gateway_name,
                support_compression,
                timestamp,
            } => {
                let timestamp = DateTimeAsMicroseconds::new(timestamp);

                gateway_connection.set_supported_compression(support_compression);

                println!(
                    "Got handshake from gateway {}. Timestamp: {}",
                    gateway_name,
                    timestamp.to_rfc3339()
                );

                let now = DateTimeAsMicroseconds::now();

                let loading_packet = now - timestamp;
                if loading_packet.get_full_seconds() > 5 {
                    return Err(format!("Handshake packet is too old. {:?}", loading_packet));
                }

                gateway_connection.set_connection_timestamp(timestamp);

                gateway_connection.set_gateway_id(gateway_name);
                tcp_gateway
                    .set_gateway_connection(gateway_name, gateway_connection.clone().into());
                gateway_connection.send_payload(&contract).await;
            }
            TcpGatewayContract::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                let remote_addr = remote_host.to_string();
                let gateway_connection = gateway_connection.clone();

                tokio::spawn(crate::tcp_gateway::scripts::handle_forward_connect(
                    connection_id,
                    remote_addr,
                    timeout,
                    gateway_connection,
                ));
            }

            TcpGatewayContract::ForwardPayload {
                connection_id,

                payload,
            } => {
                crate::tcp_gateway::scripts::forward_payload(
                    gateway_connection,
                    connection_id,
                    payload.as_slice(),
                )
                .await
            }

            TcpGatewayContract::BackwardPayload {
                connection_id,
                payload,
            } => {
                gateway_connection
                    .incoming_payload_for_proxy_connection(connection_id, payload.as_slice())
                    .await;
            }
            TcpGatewayContract::Ping => {
                gateway_connection
                    .send_payload(&TcpGatewayContract::Pong)
                    .await;
            }
            TcpGatewayContract::Pong => {}
            TcpGatewayContract::Connected { connection_id } => {
                gateway_connection
                    .notify_forward_proxy_connection_accepted(connection_id)
                    .await;
            }
            TcpGatewayContract::ConnectionError {
                connection_id,
                error,
            } => {
                println!(
                    "Gateway: [{}]. Connection error with id {}. Message: {}",
                    gateway_connection.get_gateway_id(),
                    connection_id,
                    error
                );

                gateway_connection
                    .disconnect_forward_proxy_connection(connection_id, error)
                    .await;
            }
            TcpGatewayContract::UpdatePingTime { duration } => {
                gateway_connection.last_ping_duration.update(duration);
            }
            TcpGatewayContract::GetFileRequest { path, request_id } => {
                crate::tcp_gateway::scripts::serve_file(
                    request_id,
                    path.to_string(),
                    gateway_connection.clone(),
                )
                .await
            }
            TcpGatewayContract::GetFileResponse {
                request_id,
                status,
                content,
            } => {
                gateway_connection
                    .notify_file_response(request_id, status, content)
                    .await;
            }
            TcpGatewayContract::SyncSslCertificatesRequest { cert_ids } => {
                let requested: Vec<String> = cert_ids.iter().map(|s| s.to_string()).collect();
                spawn_reply_sync_ssl_certificates(gateway_connection.clone(), requested);
            }
            TcpGatewayContract::SyncSslCertificates { .. } => {
                // Server does not expect to receive SyncSslCertificates from clients.
            }
            TcpGatewayContract::SyncSslCertificateNotFound { .. } => {
                // Server does not expect to receive SyncSslCertificateNotFound from clients.
            }
        }
        Ok(())
    }
}

fn spawn_reply_sync_ssl_certificates(
    gateway_connection: Arc<TcpGatewayConnection>,
    requested_ids: Vec<String>,
) {
    tokio::spawn(async move {
        if requested_ids.is_empty() {
            return;
        }

        let gateway_name = gateway_connection.get_gateway_id();

        let mut sent_certs = 0usize;
        let mut sent_not_found = 0usize;
        for cert_id in &requested_ids {
            let holder = if cert_id.as_str() == crate::self_signed_cert::SELF_SIGNED_CERT_NAME {
                None
            } else {
                crate::app::APP_CTX
                    .ssl_certificates_cache
                    .read(|c| {
                        c.ssl_certs
                            .get(crate::configurations::SslCertificateIdRef::new(cert_id))
                    })
                    .await
            };

            let pkt = match &holder {
                Some(h) => TcpGatewayContract::SyncSslCertificates {
                    cert_id: cert_id.as_str(),
                    cert_pem: SliceOrVec::AsSlice(h.cert_pem.as_slice()),
                    private_key_pem: SliceOrVec::AsSlice(h.private_key_pem.as_slice()),
                },
                None => TcpGatewayContract::SyncSslCertificateNotFound {
                    cert_id: cert_id.as_str(),
                },
            };

            if !gateway_connection.send_payload(&pkt).await {
                eprintln!(
                    "Gateway server: sync cert reply to [{}] failed at id={}, aborting",
                    gateway_name, cert_id
                );
                return;
            }

            if holder.is_some() {
                sent_certs += 1;
            } else {
                sent_not_found += 1;
            }
        }

        if sent_certs > 0 || sent_not_found > 0 {
            println!(
                "Gateway server: replied to client [{}] — {} cert(s), {} not-found",
                gateway_name, sent_certs, sent_not_found
            );
        }
    });
}
