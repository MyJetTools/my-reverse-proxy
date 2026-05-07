use std::sync::Arc;

use http::{HeaderMap, StatusCode};

use crate::MyHttpClientDisconnect;

pub enum MyHttpResponse<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    Response(crate::HyperResponse),
    WebSocketUpgrade {
        stream: TStream,
        response: crate::HyperResponse,
        disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    },
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    MyHttpResponse<TStream>
{
    pub fn status(&self) -> StatusCode {
        match self {
            MyHttpResponse::Response(response) => response.status(),
            MyHttpResponse::WebSocketUpgrade { response, .. } => response.status(),
        }
    }

    pub fn headers(&self) -> &HeaderMap {
        match self {
            MyHttpResponse::Response(response) => response.headers(),
            MyHttpResponse::WebSocketUpgrade { response, .. } => response.headers(),
        }
    }

    pub fn headers_mut(&mut self) -> &HeaderMap {
        match self {
            MyHttpResponse::Response(response) => response.headers_mut(),
            MyHttpResponse::WebSocketUpgrade { response, .. } => response.headers_mut(),
        }
    }

    pub fn into_response(self) -> crate::HyperResponse {
        match self {
            MyHttpResponse::Response(response) => response,
            MyHttpResponse::WebSocketUpgrade { response, .. } => response,
        }
    }
}
