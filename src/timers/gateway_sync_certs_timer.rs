use rust_extensions::{date_time::DateTimeAsMicroseconds, MyTimerTick};

use crate::{
    configurations::SslCertificateIdRef,
    ssl::SslCertificateOrigin,
    tcp_gateway::TcpGatewayContract,
};

pub struct GatewaySyncCertsTimer;

#[async_trait::async_trait]
impl MyTimerTick for GatewaySyncCertsTimer {
    async fn tick(&self) {
        let now = DateTimeAsMicroseconds::now();

        for (client_id, gateway_client) in crate::app::APP_CTX.gateway_clients.iter() {
            let sync_ids = gateway_client.get_sync_ssl_certificates();
            if sync_ids.is_empty() {
                continue;
            }

            let connections = gateway_client.get_gateway_connections().await;
            if connections.is_empty() {
                continue;
            }

            let mut need: Vec<String> = Vec::new();

            for id in sync_ids {
                let holder = crate::app::APP_CTX
                    .ssl_certificates_cache
                    .read(|c| c.ssl_certs.get(SslCertificateIdRef::new(id)))
                    .await;

                match holder {
                    None => need.push(id.clone()),
                    Some(h) => {
                        if !matches!(h.origin, SslCertificateOrigin::GatewayPushed { .. }) {
                            continue;
                        }
                        let info = h.ssl_cert.get_cert_info();
                        let days = info.expires.duration_since(now).get_full_days();
                        if days <= 1 {
                            need.push(id.clone());
                        }
                    }
                }
            }

            if need.is_empty() {
                continue;
            }

            let ids_ref: Vec<&str> = need.iter().map(|s| s.as_str()).collect();
            let request = TcpGatewayContract::SyncSslCertificatesRequest {
                cert_ids: ids_ref,
            };

            for conn in connections {
                conn.send_payload(&request).await;
            }

            println!(
                "Gateway client [{}]: requested sync for {} SSL cert(s): {:?}",
                client_id,
                need.len(),
                need
            );
        }
    }
}
