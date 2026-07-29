use std::time::Duration;

/// Per-endpoint MCP tunnel parameters. The timeouts come from the resolved
/// timeout cascade; the buffer size is endpoint-only (not part of the cascade).
#[derive(Debug, Clone, Copy)]
pub struct McpEndpointSettings {
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub buffer_size: usize,
}

impl McpEndpointSettings {
    pub fn new(read_timeout: Duration, write_timeout: Duration, buffer_size: usize) -> Self {
        Self {
            read_timeout,
            write_timeout,
            buffer_size,
        }
    }
}
