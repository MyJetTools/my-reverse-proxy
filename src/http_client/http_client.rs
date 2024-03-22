use std::sync::Arc;

use my_ssh::{SshCredentials, SshSession};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{app::AppContext, http_proxy_pass::ProxyPassError, settings::RemoteHost};

use super::{Http1Client, Http2Client, HttpClientError};

pub enum HttpClient {
    NotConnected,
    Http(Http1Client),
    Http2(Http2Client),
    Disposed(Option<DateTimeAsMicroseconds>),
}

impl HttpClient {
    pub fn new() -> Self {
        HttpClient::NotConnected
    }

    pub fn has_connection(&self) -> bool {
        match self {
            HttpClient::NotConnected => false,
            HttpClient::Disposed(_) => false,
            _ => true,
        }
    }

    pub fn unwrap_as_http1_mut(&mut self, id: i64) -> Result<&mut Http1Client, ProxyPassError> {
        match self {
            Self::Http(client) => Ok(client),
            Self::Http2(_) => panic!("Unwrapping as HTTP1 when it is HTTP2"),
            Self::NotConnected => panic!("Not Connected"),
            Self::Disposed(_) => {
                println!(
                    "HttpClient::unwrap_as_http1_mut. Connection is disposed. id: {}",
                    id
                );
                Err(ProxyPassError::ConnectionIsDisposed)
            }
        }
    }

    pub fn unwrap_as_http2_mut(&mut self, id: i64) -> Result<&mut Http2Client, ProxyPassError> {
        match self {
            Self::Http(_) => panic!("Unwrapping as HTTP2 when it is HTTP1"),
            Self::Http2(client) => Ok(client),
            Self::NotConnected => panic!("Not Connected"),
            Self::Disposed(_) => {
                println!(
                    "HttpClient::unwrap_as_http2_mut. Connection is disposed. id: {}",
                    id
                );
                Err(ProxyPassError::ConnectionIsDisposed)
            }
        }
    }

    pub async fn connect_to_http1(
        &mut self,
        remote_host: &RemoteHost,
    ) -> Result<(), HttpClientError> {
        let client = Http1Client::connect(remote_host).await?;
        *self = Self::Http(client);
        Ok(())
    }

    pub async fn connect_to_http1_over_ssh(
        &mut self,
        app: &AppContext,
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<Arc<SshSession>, ProxyPassError> {
        let (client, session) =
            Http1Client::connect_over_ssh(app, ssh_credentials, remote_host).await?;
        *self = Self::Http(client);
        Ok(session)
    }

    pub async fn connect_to_http2(&mut self, uri: &RemoteHost) -> Result<(), HttpClientError> {
        let client = Http2Client::connect(uri).await?;
        *self = Self::Http2(client);
        Ok(())
    }

    pub async fn connect_to_http2_over_ssh(
        &mut self,
        app: &AppContext,
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<Arc<SshSession>, ProxyPassError> {
        let (client, session) =
            Http2Client::connect_over_ssh(app, ssh_credentials, remote_host).await?;
        *self = Self::Http2(client);
        Ok(session)
    }

    pub fn get_connected_moment(&self) -> Option<DateTimeAsMicroseconds> {
        match self {
            HttpClient::NotConnected => None,
            HttpClient::Http(client) => Some(client.connected),
            HttpClient::Http2(client) => Some(client.connected),
            HttpClient::Disposed(connected_moment) => *connected_moment,
        }
    }

    pub fn dispose(&mut self) {
        let connected_moment = self.get_connected_moment();
        *self = HttpClient::Disposed(connected_moment);
    }
}
