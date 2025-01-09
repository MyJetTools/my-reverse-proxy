use std::sync::Arc;

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use hyper_tungstenite::{
    tungstenite::Message, HyperWebsocket, HyperWebsocketStream, WebSocketStream,
};
use my_http_client::MyHttpClientDisconnect;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub async fn start_web_socket_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    server_web_socket: HyperWebsocket,
    to_remote_stream: TStream,
    debug: bool,
    disconnect: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    trace_payload: bool,
) {
    let _ = tokio::spawn(async move {
        web_socket_loop(server_web_socket, to_remote_stream, debug, trace_payload).await;
    })
    .await;
    disconnect.web_socket_disconnect();
}

async fn web_socket_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    server_web_socket: HyperWebsocket,
    to_remote_stream: TStream,
    debug: bool,
    trace_payload: bool,
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

            tokio::spawn(serve_from_server_to_client(
                ws_receiver,
                to_remote_write,
                debug,
                trace_payload,
            ));

            if trace_payload {
                println!("WS is starting reading message from remote");
            }

            let mut have_traced_message = false;

            while let Some(message) = from_remote_read.next().await {
                let message = message;

                if message.is_err() {
                    if debug {
                        println!("Error in websocket connection: {:?}", message);
                    }

                    break;
                }

                let message = message.unwrap();

                if trace_payload {
                    if !have_traced_message {
                        println!("WS Message from remote: {:?}", message);
                        have_traced_message = true;
                    }
                }

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
) -> Result<(), Error> {
    if trace_payload {
        println!("WS is starting reading message to remote");
    }

    let mut have_traced_message = false;
    while let Some(message) = websocket.next().await {
        let msg: Message = message?;

        if trace_payload {
            if !have_traced_message {
                println!("WS Message to remote: {:?}", msg);
                have_traced_message = true;
            }
        }

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
