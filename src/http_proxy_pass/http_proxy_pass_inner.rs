use std::{net::SocketAddr, time::Duration};

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{app::AppContext, settings::HttpEndpointModifyHeadersSettings};

use super::{HostPort, LocationIndex, ProxyPassError, ProxyPassLocations, SourceHttpConfiguration};

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
    pub src: SourceHttpConfiguration,
    pub modify_headers_settings: HttpEndpointModifyHeadersSettings,
}

impl HttpProxyPassInner {
    pub fn new(
        socket_addr: SocketAddr,
        modify_headers_settings: HttpEndpointModifyHeadersSettings,
    ) -> Self {
        Self {
            locations: ProxyPassLocations::new(),
            disposed: false,
            src: SourceHttpConfiguration::new(socket_addr),
            modify_headers_settings,
        }
    }

    pub fn initialized(&self) -> bool {
        self.locations.has_configurations()
    }

    pub async fn init<'s>(
        &mut self,
        app: &AppContext,
        host_port: &HostPort<'s>,
    ) -> Result<(), ProxyPassError> {
        let configurations = crate::flows::get_configurations(app, &host_port).await?;
        self.locations.init(configurations);
        if let Some(host) = host_port.get_host() {
            self.src.host = Some(host.to_string());
        }
        self.src.is_https = host_port.is_https();
        Ok(())
    }

    pub async fn handle_error(
        &mut self,
        app: &AppContext,
        err: &ProxyPassError,
        location_index: &LocationIndex,
    ) -> Result<RetryType, ProxyPassError> {
        let mut do_retry = RetryType::NoRetry;

        if err.is_disposed() {
            println!(
                "ProxyPassInner::handle_error. Connection with id {} and index {} is disposed. Trying to reconnect",
                location_index.id,
                location_index.index
            );
            let location = self.locations.find_mut(location_index);
            location.connect_if_require(app).await?;
            return Ok(RetryType::Retry(None));
        }

        if let ProxyPassError::HyperError(err) = err {
            if err.is_canceled() {
                let location = self.locations.find_mut(location_index);

                let mut dispose_connection = false;

                if let Some(connected_moment) = location.get_connected_moment() {
                    let now = DateTimeAsMicroseconds::now();

                    if now.duration_since(connected_moment).as_positive_or_zero()
                        > OLD_CONNECTION_DELAY
                    {
                        dispose_connection = true;
                        do_retry = RetryType::Retry(None);
                    } else {
                        do_retry = RetryType::Retry(NEW_CONNECTION_NOT_READY_RETRY_DELAY.into());
                    }
                }

                if dispose_connection {
                    location.dispose();
                    location.connect_if_require(app).await?;
                }
            }
        }

        Ok(do_retry)
    }
}
