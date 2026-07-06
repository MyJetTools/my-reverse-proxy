use mcp_server_middleware::*;
use serde::*;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetSslEndpointsStatusInputData {}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct SslEndpointStatusEntry {
    #[property(description = "Endpoint host as configured, e.g. `example.com:443` or a unix socket path.")]
    pub endpoint_host: String,

    #[property(description = "The domain (SNI server name) the endpoint protects. Empty for a default endpoint with no server name.")]
    pub server_name: String,

    #[property(description = "Id of the SSL certificate this endpoint references. Use this as `cert_id` when uploading a replacement via init_ssl_certificate.")]
    pub cert_id: String,

    #[property(description = "Certificate status: `missing` (not loaded — handshake fails now), `expired`, `expiring_soon` (<= 7 days), or `ok`.")]
    pub status: String,

    #[property(description = "True when the certificate is missing or expired and the endpoint needs a new certificate to serve TLS.")]
    pub needs_new_certificate: bool,

    #[property(description = "Common Name of the loaded certificate. Empty when the certificate is missing.")]
    pub cn: String,

    #[property(description = "RFC3339 expiry of the loaded certificate. Empty when the certificate is missing.")]
    pub expires: String,

    #[property(description = "Whole days until expiry of the loaded certificate (negative if already expired). 0 when the certificate is missing.")]
    pub days_to_expire: i64,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetSslEndpointsStatusResponse {
    #[property(description = "One entry per https/mcp endpoint that references a real (non self-signed) SSL certificate.")]
    pub endpoints: Vec<SslEndpointStatusEntry>,

    #[property(description = "Total number of endpoints reported.")]
    pub total: i64,

    #[property(description = "How many endpoints have a missing or expired certificate and need a new one uploaded.")]
    pub needs_attention: i64,
}

pub struct GetSslEndpointsStatusHandler;

impl ToolDefinition for GetSslEndpointsStatusHandler {
    const FUNC_NAME: &'static str = "get_ssl_endpoints_status";
    const DESCRIPTION: &'static str = "List every https/mcp endpoint that uses a real (non self-signed) SSL certificate and report whether that certificate is missing (declared in configuration but never resolved — its TLS handshake fails right now), expired, expiring soon (<= 7 days), or ok. Use this to find which endpoints need a fresh certificate, together with the `cert_id` and the domain each one protects. To fix a missing/expired certificate, obtain a PEM certificate + private key that is valid for that endpoint's `server_name` and upload it with init_ssl_certificate — it is served immediately, no reload required.";
}

#[async_trait::async_trait]
impl McpToolCall<GetSslEndpointsStatusInputData, GetSslEndpointsStatusResponse>
    for GetSslEndpointsStatusHandler
{
    async fn execute_tool_call(
        &self,
        _model: GetSslEndpointsStatusInputData,
    ) -> Result<GetSslEndpointsStatusResponse, String> {
        let statuses = crate::scripts::get_ssl_endpoints_status().await;

        let mut needs_attention = 0i64;
        let mut endpoints = Vec::with_capacity(statuses.len());

        for status in statuses {
            let needs_new_certificate = status.status.needs_attention();
            if needs_new_certificate {
                needs_attention += 1;
            }

            endpoints.push(SslEndpointStatusEntry {
                endpoint_host: status.endpoint_host,
                server_name: status.server_name.unwrap_or_default(),
                cert_id: status.cert_id,
                status: status.status.as_str().to_string(),
                needs_new_certificate,
                cn: status.cn.unwrap_or_default(),
                expires: status
                    .expires
                    .map(|e| e.to_rfc3339())
                    .unwrap_or_default(),
                days_to_expire: status.days_to_expire.unwrap_or(0),
            });
        }

        let total = endpoints.len() as i64;

        Ok(GetSslEndpointsStatusResponse {
            endpoints,
            total,
            needs_attention,
        })
    }
}
