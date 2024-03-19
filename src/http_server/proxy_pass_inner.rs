use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http_body_util::Full;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::app::AppContext;

use super::{ProxyPassConfiguration, ProxyPassError};

const OLD_CONNECTION_DELAY: Duration = Duration::from_secs(10);

const NEW_CONNECTION_NOT_READY_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum RetryType {
    Retry(Option<Duration>),
    NoRetry,
}

pub enum ProxyPassInner {
    Unknown,
    Active(Vec<ProxyPassConfiguration>),
    Disposed,
}

impl ProxyPassInner {
    async fn get_configurations<'s>(
        &'s mut self,
        app: &Arc<AppContext>,
        req: &hyper::Request<Full<Bytes>>,
    ) -> Result<&'s mut ProxyPassConfiguration, ProxyPassError> {
        match self {
            Self::Active(configurations) => {
                let result =
                    crate::flows::find_proxy_pass_by_uri(app, configurations, req.uri()).await?;

                Ok(result)
            }
            _ => Err(ProxyPassError::NoLocationFound),
        }
    }

    pub async fn get_proxy_pass_configuration<'s>(
        &'s mut self,
        app: &Arc<AppContext>,
        req: &hyper::Request<Full<Bytes>>,
    ) -> Result<&mut ProxyPassConfiguration, ProxyPassError> {
        match self {
            Self::Unknown => {}
            Self::Disposed => return Err(ProxyPassError::ConnectionIsDisposed),
            Self::Active(configurations) => {
                let result =
                    crate::flows::find_proxy_pass_by_uri(app, configurations, req.uri()).await?;

                return Ok(result);
            }
        };
        let host_port = crate::http_server::HostPort::new(req);

        let configs = crate::flows::get_configurations(app, &host_port).await?;

        *self = Self::Active(configs);

        self.get_configurations(app, req).await
    }

    pub async fn handle_error(
        &mut self,
        err: &hyper::Error,
        proxy_pass_id: i64,
    ) -> Result<bool, ProxyPassError> {
        match self {
            Self::Unknown => {}
            Self::Disposed => {
                return Err(ProxyPassError::ConnectionIsDisposed);
            }
            Self::Active(configurations) => {
                let mut do_retry = RetryType::NoRetry;

                if err.is_canceled() {
                    let mut found_proxy_pass = None;
                    for proxy_pass in configurations.iter_mut() {
                        if proxy_pass.id == proxy_pass_id {
                            found_proxy_pass = Some(proxy_pass);
                            break;
                        }
                    }

                    if let Some(found_proxy_pass) = found_proxy_pass {
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

                println!(
                    "{}: Retry: {:?}, Error: {:?}",
                    DateTimeAsMicroseconds::now().to_rfc3339(),
                    do_retry,
                    err
                );

                match do_retry {
                    RetryType::Retry(delay) => {
                        if let Some(delay) = delay {
                            tokio::time::sleep(delay).await;
                        }
                    }
                    RetryType::NoRetry => {
                        return Ok(true);
                        // return Ok(Err(err.into()));
                    }
                }
            }
        }

        Ok(false)
    }
}
