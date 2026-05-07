use std::time::Duration;

use crate::network_stream::*;

#[async_trait::async_trait]
pub trait H1Writer {
    async fn write_http_payload(
        &mut self,
        request_id: u64,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError>;
}
