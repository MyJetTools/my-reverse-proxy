use mcp_server_middleware::*;
use serde::*;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetSslCertificateInputData {
    #[property(description = "Id of the certificate to read (see the `cert_id` field of get_ssl_endpoints_status).")]
    pub cert_id: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetSslCertificateResponse {
    #[property(description = "True when a certificate is loaded under this id. When false, all other fields are empty and the endpoint's TLS handshake currently fails.")]
    pub found: bool,

    #[property(description = "Id of the certificate.")]
    pub cert_id: String,

    #[property(description = "Common Name of the certificate.")]
    pub cn: String,

    #[property(description = "RFC3339 expiry of the certificate.")]
    pub expires: String,

    #[property(description = "Where the certificate came from: `local_source`, `gateway_pushed` or `manually_provided`.")]
    pub origin: String,

    #[property(description = "Domains (SAN DNS names + CN) the certificate is valid for.")]
    pub domains: Vec<String>,
}

pub struct GetSslCertificateHandler;

impl ToolDefinition for GetSslCertificateHandler {
    const FUNC_NAME: &'static str = "get_ssl_certificate";
    const DESCRIPTION: &'static str = "Return metadata about the currently loaded SSL certificate for a given id: its CN, expiry, the domains it is valid for, and where it came from. Metadata only — the raw certificate PEM material and the private key are never exposed (not over MCP, not over REST). Use this to inspect what is being served for an endpoint, e.g. before uploading a replacement with init_ssl_certificate.";
}

#[async_trait::async_trait]
impl McpToolCall<GetSslCertificateInputData, GetSslCertificateResponse> for GetSslCertificateHandler {
    async fn execute_tool_call(
        &self,
        model: GetSslCertificateInputData,
    ) -> Result<GetSslCertificateResponse, String> {
        match crate::scripts::get_ssl_certificate_details(&model.cert_id).await {
            Some(details) => Ok(GetSslCertificateResponse {
                found: true,
                cert_id: details.id,
                cn: details.cn,
                expires: details.expires.to_rfc3339(),
                origin: details.origin,
                domains: details.domains,
            }),
            None => Ok(GetSslCertificateResponse {
                found: false,
                cert_id: model.cert_id.trim().to_string(),
                cn: String::new(),
                expires: String::new(),
                origin: String::new(),
                domains: Vec::new(),
            }),
        }
    }
}
