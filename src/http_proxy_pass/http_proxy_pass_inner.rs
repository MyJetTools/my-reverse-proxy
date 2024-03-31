use std::{net::SocketAddr, time::Duration};

use rust_extensions::{
    date_time::DateTimeAsMicroseconds, placeholders::PlaceholdersIterator, StrOrString,
};

use crate::{
    app::AppContext,
    populate_variable::{PLACEHOLDER_CLOSE_TOKEN, PLACEHOLDER_OPEN_TOKEN},
    settings::HttpEndpointModifyHeadersSettings,
};

use super::{
    AllowedUserList, HostPort, HttpProxyPassContentSource, HttpProxyPassIdentity,
    HttpRequestBuilder, HttpServerConnectionInfo, LocationIndex, ProxyPassError,
    ProxyPassLocations, SourceHttpData,
};

const OLD_CONNECTION_DELAY: Duration = Duration::from_secs(10);

const NEW_CONNECTION_NOT_READY_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum RetryType {
    Retry(Option<Duration>),
    NoRetry,
}

pub struct HttpProxyPassInner {
    pub locations: ProxyPassLocations,
    pub disposed: bool,
    pub src: SourceHttpData,
    pub modify_headers_settings: HttpEndpointModifyHeadersSettings,
    pub allowed_user_list: Option<AllowedUserList>,

    pub identity: HttpProxyPassIdentity,
}

impl HttpProxyPassInner {
    pub fn new(
        socket_addr: SocketAddr,
        modify_headers_settings: HttpEndpointModifyHeadersSettings,
        client_cert_cn: Option<String>,
    ) -> Self {
        Self {
            locations: ProxyPassLocations::new(),
            disposed: false,
            src: SourceHttpData::new(socket_addr),
            modify_headers_settings,
            allowed_user_list: None,
            identity: HttpProxyPassIdentity::new(client_cert_cn),
        }
    }

    pub fn initialized(&self) -> bool {
        self.locations.has_configurations()
    }

    pub async fn init<'s>(
        &mut self,
        app: &AppContext,
        endpoint_info: &HttpServerConnectionInfo,
        req: &HttpRequestBuilder,
    ) -> Result<(), ProxyPassError> {
        let (locations, allowed_user_list) =
            crate::flows::get_locations(app, endpoint_info, req).await?;

        self.locations.init(locations);
        self.src.is_https = endpoint_info.http_type.is_https();
        self.allowed_user_list = allowed_user_list;
        Ok(())
    }

    pub async fn handle_error(
        &mut self,
        app: &AppContext,
        err: &ProxyPassError,
        location_index: &LocationIndex,
        debug: bool,
    ) -> Result<RetryType, ProxyPassError> {
        let mut do_retry = RetryType::NoRetry;

        if err.is_disposed() {
            if debug {
                println!(
                    "ProxyPassInner::handle_error. Connection with id {} and index {} is disposed. Trying to reconnect",
                    location_index.id,
                    location_index.index
                );
            }
            let location = self.locations.find_mut(location_index);
            location.connect_if_require(app, debug).await?;
            return Ok(RetryType::Retry(None));
        }

        if let ProxyPassError::HyperError(err) = err {
            if err.is_canceled() {
                let location = self.locations.find_mut(location_index);

                match &mut location.content_source {
                    HttpProxyPassContentSource::Http(remote_http_content_source) => {
                        let mut dispose_connection = false;

                        if let Some(connected_moment) =
                            remote_http_content_source.get_connected_moment()
                        {
                            let now = DateTimeAsMicroseconds::now();

                            if now.duration_since(connected_moment).as_positive_or_zero()
                                > OLD_CONNECTION_DELAY
                            {
                                dispose_connection = true;
                                do_retry = RetryType::Retry(None);
                            } else {
                                do_retry =
                                    RetryType::Retry(NEW_CONNECTION_NOT_READY_RETRY_DELAY.into());
                            }
                        }

                        if dispose_connection {
                            remote_http_content_source.dispose();
                            remote_http_content_source
                                .connect_if_require(app, debug)
                                .await?;
                        }
                    }
                    HttpProxyPassContentSource::LocalPath(_) => {}
                    HttpProxyPassContentSource::PathOverSsh(_) => {}
                    HttpProxyPassContentSource::Static(_) => {}
                }
            }
        }

        Ok(do_retry)
    }

    pub fn populate_value<'s, THostPort: HostPort + Send + Sync + 'static>(
        &'s self,
        value: &'s str,
        req_host_port: &THostPort,
    ) -> StrOrString<'s> {
        if !value.contains(PLACEHOLDER_OPEN_TOKEN) {
            return value.into();
        }

        let mut result = String::new();

        for token in
            PlaceholdersIterator::new(value, PLACEHOLDER_OPEN_TOKEN, PLACEHOLDER_CLOSE_TOKEN)
        {
            match token {
                rust_extensions::placeholders::ContentToken::Text(text) => result.push_str(text),
                rust_extensions::placeholders::ContentToken::Placeholder(placeholder) => {
                    match placeholder {
                        "HOST" => {
                            if let Some(host) = req_host_port.get_host() {
                                result.push_str(host);
                            }
                        }
                        "ENDPOINT_IP" => {
                            result.push_str(format!("{}", self.src.socket_addr.ip()).as_str());
                        }

                        "PATH_AND_QUERY" => {
                            if let Some(value) = req_host_port.get_path_and_query() {
                                result.push_str(value);
                            }
                        }

                        "HOST_PORT" => {
                            if let Some(host) = req_host_port.get_host() {
                                result.push_str(host);
                                if let Some(port) = req_host_port.get_port() {
                                    result.push(':');
                                    result.push_str(port.to_string().as_str());
                                }
                            }
                        }

                        "CLIENT_CERT_CN" => {
                            if let Some(value) = self.identity.get_identity() {
                                result.push_str(value);
                            }
                        }

                        "ENDPOINT_SCHEMA" => {
                            if self.src.is_https {
                                result.push_str("https");
                            } else {
                                result.push_str("http");
                            }
                        }
                        _ => {
                            if let Ok(value) = std::env::var(placeholder) {
                                result.push_str(&value);
                            }
                        }
                    }
                }
            }
        }

        result.into()
    }
}
