use std::{panic::AssertUnwindSafe, sync::Arc, time::Duration};

use futures::{
    stream::{SplitSink, SplitStream},
    FutureExt, SinkExt, StreamExt,
};
use hyper_tungstenite::{
    tungstenite::Message, HyperWebsocket, HyperWebsocketStream, WebSocketStream,
};
use my_http_client::MyHttpClientDisconnect;

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
    debug: bool,
    disconnect: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    trace_payload: bool,
    domain: String,
) {
    let result = AssertUnwindSafe(async move {
        web_socket_loop(
            server_web_socket,
            to_remote_stream,
            debug,
            trace_payload,
            domain,
        )
        .await;
    })
    .catch_unwind()
    .await;

    if result.is_err() && debug {
        println!("ws_loop_main panicked");
    }

    disconnect.web_socket_disconnect();
}

async fn web_socket_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    server_web_socket: HyperWebsocket,
    to_remote_stream: TStream,
    debug: bool,
    trace_payload: bool,
    domain: String,
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
                    debug,
                    trace_payload,
                    domain.clone(),
                ),
            );

            if trace_payload {
                println!("WS is starting reading message from remote");
            }

            let mut have_traced_message = false;

            let read_timeout = Duration::from_secs(60);

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
                    if debug {
                        println!("Error in websocket connection: {:?}", message);
                    }

                    break;
                }

                let message = message.unwrap();

                if trace_payload {
                    if !have_traced_message {
                        //println!("WS Message from remote: {:?}", message);
                        have_traced_message = true;
                    }
                }

                let bytes = ws_message_payload_len(&message);
                crate::app::APP_CTX.traffic.record_ws_s2c(&domain, bytes);

                if let Err(err) = ws_sender.send(message).await {
                    if debug {
                        println!("ws_sender.send error: {:?}", err);
                    }

                    break;
                }
            }
        }
        Err(err) => {
            if debug {
                println!("Error in websocket connection: {}", err);
            }
        }
    }
}

async fn serve_from_server_to_client<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    mut websocket: SplitStream<HyperWebsocketStream>,
    mut to_remote_write: SplitSink<WebSocketStream<TStream>, Message>,
    debug: bool,
    trace_payload: bool,
    domain: String,
) -> Result<(), Error> {
    if trace_payload {
        println!("WS is starting reading message to remote");
    }

    let mut have_traced_message = false;

    let read_timeout = Duration::from_secs(60);
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
                println!("WS Message to remote: {:?}", msg);
                have_traced_message = true;
            }
        }

        let bytes = ws_message_payload_len(&msg);
        crate::app::APP_CTX.traffic.record_ws_c2s(&domain, bytes);

        let err = to_remote_write.send(msg).await;
        if let Err(err) = err {
            if debug {
                println!("Error in websocket server_to_client loop: {:?}", err);
            }
            break;
        }
    }

    Ok(())
}
