use hyper_tungstenite::HyperWebsocket;

pub enum HyperHttpResponse {
    Response(crate::HyperResponse),
    WebSocketUpgrade {
        response: crate::HyperResponse,
        web_socket: HyperWebsocket,
    },
}
