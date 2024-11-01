use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use hyper_tungstenite::{
    tungstenite::Message, HyperWebsocket, HyperWebsocketStream, WebSocketStream,
};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub async fn web_socket_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    server_web_socket: HyperWebsocket,
    to_remote_stream: TStream,
    debug: bool,
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

            tokio::spawn(serve_from_server_to_client(ws_receiver, to_remote_write));

            while let Some(message) = from_remote_read.next().await {
                let message = message.unwrap();

                if let Err(err) = ws_sender.send(message).await {
                    if debug {
                        println!("ws_sender.send error: {:?}", err);
                    }

                    return;
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
) -> Result<(), Error> {
    while let Some(message) = websocket.next().await {
        let msg: Message = message?;

        to_remote_write.send(msg).await.unwrap();
    }

    Ok(())
}
