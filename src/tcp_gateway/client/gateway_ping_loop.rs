use std::{sync::Arc, time::Duration};

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::tcp_gateway::{TcpGatewayConnection, TcpGatewayContract};

const PING_DELAY: Duration = Duration::from_secs(3);

pub async fn gateway_ping_loop(gateway_connection: Arc<TcpGatewayConnection>, debug: bool) {
    loop {
        tokio::time::sleep(PING_DELAY).await;
        let now = DateTimeAsMicroseconds::now();

        let incoming_interval = now - gateway_connection.get_last_incoming_payload_time();

        let incoming_interval_sec = incoming_interval.get_full_seconds();

        if incoming_interval_sec > 9 {
            if debug {
                println!(
                    "Detected dead Tcp Gateway connection to {} with id {}",
                    gateway_connection.addr.as_str(),
                    gateway_connection.gateway_id.as_str()
                );
            }

            gateway_connection.disconnect_gateway().await;
            break;
        }

        if incoming_interval.get_full_seconds() > 3 {
            let sent_ok = gateway_connection
                .send_payload(&TcpGatewayContract::Ping)
                .await;

            if !sent_ok {
                break;
            }
        }
    }
}
