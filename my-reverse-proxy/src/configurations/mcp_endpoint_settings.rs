use std::time::Duration;

use crate::settings::EndpointSettings;

/// Per-endpoint MCP tunnel parameters, resolved from the endpoint config with
/// fallback to the hardcoded defaults in `crate::consts`.
#[derive(Debug, Clone, Copy)]
pub struct McpEndpointSettings {
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub buffer_size: usize,
}

impl McpEndpointSettings {
    pub fn from_settings(endpoint: &EndpointSettings) -> Self {
        Self {
            read_timeout: endpoint.get_mcp_read_timeout(),
            write_timeout: endpoint.get_mcp_write_timeout(),
            buffer_size: endpoint.get_mcp_buffer_size(),
        }
    }
}
