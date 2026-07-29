use std::{panic::AssertUnwindSafe, sync::Arc};

use futures::{
    stream::{SplitSink, SplitStream},
    FutureExt, SinkExt, StreamExt,
};
use hyper_tungstenite::{
    tungstenite::Message, HyperWebsocket, HyperWebsocketStream, WebSocketStream,
};
use my_http_client::MyHttpClientDisconnect;

use crate::types::HttpTimeouts;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

fn ws_message_payload_len(msg: &Message) -> u64 {
    match msg {
        Message::Text(s) => s.len() as u64,
        Message::Binary(b) => b.len() as u64,
        Message::Ping(b) | Message::Pong(b) => b.len() as u64,
        Message::Close(_) => 0,
        Message::Frame(f) => f.payload().len() as u64,
    }
}

pub async fn start_web_socket_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    server_web_socket: HyperWebsocket,
    to_remote_stream: TStream,
    log_scope: crate::app::ProxyLogScope,
    disconnect: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    trace_payload: bool,
    domain: Option<String>,
    timeouts: HttpTimeouts,
) {
    let panic_log_scope = log_scope.clone();
    let result = AssertUnwindSafe(async move {
        web_socket_loop(
            server_web_socket,
            to_remote_stream,
            log_scope,
            trace_payload,
            domain,
            timeouts,
        )
        .await;
    })
    .catch_unwind()
    .await;

    if result.is_err() {
        panic_log_scope.write("ws_loop_main panicked".to_string());
    }

    disconnect.web_socket_disconnect();
}

async fn web_socket_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    server_web_socket: HyperWebsocket,
    to_remote_stream: TStream,
    log_scope: crate::app::ProxyLogScope,
    trace_payload: bool,
    domain: Option<String>,
    timeouts: HttpTimeouts,
) {
    let ws_stream = server_web_socket.await;

    let to_remote = WebSocketStream::from_raw_socket(
        to_remote_stream,
        hyper_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await;

    let (to_remote_write, mut from_remote_read) = to_remote.split();

    match ws_stream {
        Ok(ws_stream) => {
            let (mut ws_sender, ws_receiver) = ws_stream.split();

            crate::app::spawn_named(
                "ws_loop_server_to_client",
                serve_from_server_to_client(
                    ws_receiver,
                    to_remote_write,
                    log_scope.clone(),
                    trace_payload,
                    domain.clone(),
                    timeouts,
                ),
            );

            if trace_payload {
                log_scope.write("WS is starting reading message from remote".to_string());
            }

            let mut have_traced_message = false;

            let read_timeout = timeouts.read_timeout;
            let write_timeout = timeouts.write_timeout;

            loop {
                let future = from_remote_read.next();

                let result = tokio::time::timeout(read_timeout, future).await;

                if result.is_err() {
                    break;
                }

                let result = result.unwrap();

                if result.is_none() {
                    break;
                }

                let message = result.unwrap();

                if message.is_err() {
                    log_scope.write(format!("Error in websocket connection: {:?}", message));

                    break;
                }

                let message = message.unwrap();

                if trace_payload {
                    if !have_traced_message {
                        //println!("WS Message from remote: {:?}", message);
                        have_traced_message = true;
                    }
                }

                if let Some(d) = domain.as_deref() {
                    let bytes = ws_message_payload_len(&message);
                    crate::app::APP_CTX.traffic.record_ws_s2c(d, bytes);
                }

                match tokio::time::timeout(write_timeout, ws_sender.send(message)).await {
                    Err(_) => {
                        log_scope.write(format!(
                            "ws_sender.send timed out after {:?}",
                            write_timeout
                        ));
                        break;
                    }
                    Ok(Err(err)) => {
                        log_scope.write(format!("ws_sender.send error: {:?}", err));
                        break;
                    }
                    Ok(Ok(())) => {}
                }
            }
        }
        Err(err) => {
            log_scope.write(format!("Error in websocket connection: {}", err));
        }
    }
}

async fn serve_from_server_to_client<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    mut websocket: SplitStream<HyperWebsocketStream>,
    mut to_remote_write: SplitSink<WebSocketStream<TStream>, Message>,
    log_scope: crate::app::ProxyLogScope,
    trace_payload: bool,
    domain: Option<String>,
    timeouts: HttpTimeouts,
) -> Result<(), Error> {
    if trace_payload {
        log_scope.write("WS is starting reading message to remote".to_string());
    }

    let mut have_traced_message = false;

    let read_timeout = timeouts.read_timeout;
    let write_timeout = timeouts.write_timeout;
    loop {
        let future = websocket.next();

        let result = tokio::time::timeout(read_timeout, future).await;

        if result.is_err() {
            break;
        }

        let next_one = result.unwrap();

        if next_one.is_none() {
            break;
        }

        let message = next_one.unwrap();

        let msg: Message = message?;

        if trace_payload {
            if !have_traced_message {
                log_scope.write(format!("WS Message to remote: {:?}", msg));
                have_traced_message = true;
            }
        }

        if let Some(d) = domain.as_deref() {
            let bytes = ws_message_payload_len(&msg);
            crate::app::APP_CTX.traffic.record_ws_c2s(d, bytes);
        }

        match tokio::time::timeout(write_timeout, to_remote_write.send(msg)).await {
            Err(_) => {
                log_scope.write(format!(
                    "to_remote_write.send timed out after {:?} in server_to_client loop",
                    write_timeout
                ));
                break;
            }
            Ok(Err(err)) => {
                log_scope.write(format!(
                    "Error in websocket server_to_client loop: {:?}",
                    err
                ));
                break;
            }
            Ok(Ok(())) => {}
        }
    }

    Ok(())
}
