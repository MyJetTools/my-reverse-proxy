//! Tests for the new session-based gateway pipeline.

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use encryption::aes::AesKey;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    use crate::network_stream::MyOwnedWriteHalf;
    use crate::tcp_gateway::session::{
        accept_gateway_session, gateway_write_loop, ControlHandlerConfig, GatewaySupervisor,
        HandshakeReplayGuard, SupervisorConfig,
    };

    async fn ephemeral_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connect = TcpStream::connect(addr);
        let accept = async move {
            let (s, _) = listener.accept().await.unwrap();
            s
        };
        let (client, server) = tokio::join!(connect, accept);
        (client.unwrap(), server)
    }

    fn shared_key() -> Arc<AesKey> {
        // AesKey requires exactly 48 bytes.
        let key: [u8; 48] = *b"phase2-test-key-not-secret--padding-to-48-byte!!";
        Arc::new(AesKey::new(&key))
    }

    /// 10_000 small payloads pushed concurrently into write_tx must all arrive
    /// in order on the peer with no losses and no hangs.
    #[tokio::test(flavor = "current_thread")]
    async fn write_loop_high_pressure_no_hang() {
        let (client_stream, server_stream) = ephemeral_pair().await;
        let (_client_read, client_write) = client_stream.into_split();
        let (mut server_read, _server_write) = server_stream.into_split();

        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
        let writer = tokio::spawn(gateway_write_loop(MyOwnedWriteHalf::Tcp(client_write), rx));

        const N: u32 = 10_000;
        let producer_tx = tx.clone();
        let producer = tokio::spawn(async move {
            for i in 0..N {
                producer_tx.send(i.to_le_bytes().to_vec()).await.unwrap();
            }
        });

        let reader = tokio::spawn(async move {
            let mut received = vec![0u8; (N as usize) * 4];
            let mut filled = 0usize;
            while filled < received.len() {
                let n = server_read.read(&mut received[filled..]).await.unwrap();
                if n == 0 {
                    panic!("peer closed early at byte {}", filled);
                }
                filled += n;
            }
            for i in 0..N {
                let off = (i as usize) * 4;
                let v = u32::from_le_bytes(received[off..off + 4].try_into().unwrap());
                assert_eq!(v, i, "sequence mismatch at {}", i);
            }
        });

        producer.await.unwrap();
        drop(tx);

        let outcome = tokio::time::timeout(Duration::from_secs(10), async {
            reader.await.unwrap();
            writer.await.unwrap();
        })
        .await;

        assert!(outcome.is_ok(), "write_loop hung under load");
    }

    /// End-to-end: client supervisor connects to a server-side accept loop;
    /// `connect_to_remote()` reaches the peer, peer opens a real downstream
    /// echo socket, and bytes round-trip through the gateway in both
    /// directions.
    #[tokio::test(flavor = "current_thread")]
    async fn end_to_end_connect_and_echo() {
        // 1. Echo server (the "real downstream" service).
        let echo_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let echo_addr = echo_listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = echo_listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    loop {
                        let n = match sock.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => n,
                        };
                        if sock.write_all(&buf[..n]).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });

        // 2. Gateway listener — the server-side gateway TCP entry point.
        let gw_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gw_addr = gw_listener.local_addr().unwrap();
        let server_key = shared_key();
        let replay_guard: Arc<HandshakeReplayGuard> =
            Arc::new(HandshakeReplayGuard::default_window());
        let server_replay_guard = replay_guard.clone();
        tokio::spawn(async move {
            let (sock, _) = gw_listener.accept().await.unwrap();
            let _ = accept_gateway_session(
                sock,
                1, // session_id on this side
                "server",
                server_key,
                false,
                Duration::from_secs(5),
                1024,
                256,
                ControlHandlerConfig::default(),
                server_replay_guard,
            )
            .await
            .unwrap();
            // Keep the SessionTasks struct alive forever so its tasks aren't
            // dropped (they only stop on socket close).
            std::future::pending::<()>().await;
        });

        // 3. Client supervisor pointing at the gateway listener.
        let supervisor = GatewaySupervisor::spawn(SupervisorConfig {
            gateway_id: "client".to_string(),
            remote_host: gw_addr.to_string(),
            aes_key: shared_key(),
            support_compression: false,
            connect_timeout: Duration::from_secs(5),
            reconnect_delay: Duration::from_secs(1),
            write_channel_capacity: 1024,
            control_channel_capacity: 256,
            control_handler: ControlHandlerConfig::default(),
            replay_guard: replay_guard.clone(),
        });

        // 4. Wait for the supervisor to publish an active session.
        let session = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(s) = supervisor.current() {
                    return s;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("supervisor never produced a session");

        // 5. Use `connect_to_remote` to open a forwarded TCP via the gateway
        // to the echo server.
        let mut handle = session
            .connect_to_remote(echo_addr.to_string(), Duration::from_secs(5))
            .await
            .expect("connect_to_remote failed");

        // 6. Round-trip a payload.
        let payload = b"hello-from-gateway".to_vec();
        handle.send(payload.clone()).await.unwrap();

        let received = tokio::time::timeout(Duration::from_secs(5), handle.recv())
            .await
            .expect("recv timed out")
            .expect("data_rx closed");
        assert_eq!(received, payload);

        // 7. Clean up.
        supervisor.stop();
    }
}
