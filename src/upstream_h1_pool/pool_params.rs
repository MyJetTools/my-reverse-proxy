use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PoolParams {
    pub pool_size: u8,
    /// Reserved for symmetry with h2-pool. Active probe for h1 is not
    /// implemented — supervisor only refills empty slots.
    #[allow(dead_code)]
    pub health_check_path: Option<String>,
    pub connect_timeout: Duration,
}

impl Default for PoolParams {
    fn default() -> Self {
        Self {
            pool_size: 5,
            health_check_path: None,
            connect_timeout: Duration::from_secs(5),
        }
    }
}
