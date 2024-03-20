use std::time::Duration;

use rust_extensions::date_time::DateTimeAsMicroseconds;

use super::{ProxyPassConfigurations, ProxyPassError};

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
}

impl ProxyPassInner {
    pub fn new() -> Self {
        Self {
            configurations: ProxyPassConfigurations::new(),
            disposed: false,
        }
    }

    pub async fn handle_error(
        &mut self,
        err: &hyper::Error,
        proxy_pass_id: i64,
    ) -> Result<bool, ProxyPassError> {
        let mut do_retry = RetryType::NoRetry;

        if err.is_canceled() {
            let mut found_proxy_pass = None;
            for proxy_pass in self.configurations.iter_mut() {
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
                        do_retry = RetryType::Retry(NEW_CONNECTION_NOT_READY_RETRY_DELAY.into());
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

        Ok(false)
    }
}
