use std::{collections::HashMap, net::SocketAddr, time::Duration};

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::app::AppContext;

use super::{HostPort, ProxyPassConfigurations, ProxyPassError, SourceHttpConfiguration};

const OLD_CONNECTION_DELAY: Duration = Duration::from_secs(10);

const NEW_CONNECTION_NOT_READY_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum RetryType {
    Retry(Option<Duration>),
    NoRetry,
}

pub struct ProxyPassInner {
    pub configurations: ProxyPassConfigurations,
    pub disposed: bool,
    pub src: SourceHttpConfiguration,
    pub populate_request_headers: Option<HashMap<String, String>>,
}

impl ProxyPassInner {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            configurations: ProxyPassConfigurations::new(),
            disposed: false,
            src: SourceHttpConfiguration::new(socket_addr),
            populate_request_headers: None,
        }
    }

    pub fn update_src_info<'s>(&mut self, host: &HostPort<'s>) {
        if let Some(host) = host.get_host() {
            self.src.host = Some(host.to_string());
        }

        self.src.is_https = host.is_https();
    }

    pub async fn handle_error(
        &mut self,
        app: &AppContext,
        err: &ProxyPassError,
        proxy_pass_id: i64,
    ) -> Result<RetryType, ProxyPassError> {
        let mut do_retry = RetryType::NoRetry;

        if err.is_disposed() {
            println!(
                "ProxyPassInner::handle_error. Connection is disposed. id: {}. Trying to reconnect",
                proxy_pass_id
            );
            if let Some(found_proxy_pass) = self.configurations.find_by_id(proxy_pass_id) {
                found_proxy_pass.connect_if_require(app).await?;
                return Ok(RetryType::Retry(None));
            }
        }

        if let ProxyPassError::HyperError(err) = err {
            if err.is_canceled() {
                if let Some(found_proxy_pass) = self.configurations.find_by_id(proxy_pass_id) {
                    let mut dispose_connection = false;

                    if let Some(connected_moment) = found_proxy_pass.get_connected_moment() {
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
                        found_proxy_pass.dispose();
                    }
                }
            }
        }

        Ok(do_retry)
    }
}
