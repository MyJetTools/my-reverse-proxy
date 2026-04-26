use std::sync::Arc;

use my_http_client::{MyHttpClientConnector, MyHttpClientError};
use rust_extensions::remote_endpoint::{RemoteEndpoint, RemoteEndpointOwned};
use tokio::io::{ReadHalf, WriteHalf};

pub struct UnixSocketHttpConnector {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<tokio::net::UnixStream> for UnixSocketHttpConnector {
    fn get_remote_endpoint<'s>(&'s self) -> RemoteEndpoint<'s> {
        self.remote_endpoint.to_ref()
    }

    fn is_debug(&self) -> bool {
        self.debug
    }

    async fn connect(&self) -> Result<tokio::net::UnixStream, MyHttpClientError> {
        let host_port = self.remote_endpoint.get_host_port();
        let socket_path = expand_tilde(host_port.as_str());

        let connect_feature = tokio::net::UnixStream::connect(socket_path.as_str());

        match connect_feature.await {
            Ok(tcp_stream) => Ok(tcp_stream),
            Err(err) => Err(
                my_http_client::MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "{}. Err:{}",
                    self.remote_endpoint.as_str(),
                    err
                )),
            ),
        }
    }

    fn reunite(
        read: ReadHalf<tokio::net::UnixStream>,
        write: WriteHalf<tokio::net::UnixStream>,
    ) -> tokio::net::UnixStream {
        read.unsplit(write)
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home.trim_end_matches('/'), stripped);
        }
    } else if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return home;
        }
    }
    path.to_string()
}
