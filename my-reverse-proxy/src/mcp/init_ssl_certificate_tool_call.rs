use mcp_server_middleware::*;
use serde::*;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct InitSslCertificateInputData {
    #[property(description = "Id of the certificate to install, exactly as referenced by an endpoint (see the `cert_id` field of get_ssl_endpoints_status).")]
    pub cert_id: String,

    #[property(description = "The full certificate chain in PEM format (-----BEGIN CERTIFICATE----- ... one or more blocks). It MUST be valid for the endpoint's domain — this is verified before anything is stored.")]
    pub certificate: String,

    #[property(description = "The private key in PEM format (-----BEGIN PRIVATE KEY----- ...) matching the certificate.")]
    pub private_key: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct InitSslCertificateResponse {
    #[property(description = "Id of the certificate that was installed.")]
    pub cert_id: String,

    #[property(description = "Common Name of the installed certificate.")]
    pub cn: String,

    #[property(description = "RFC3339 expiry of the installed certificate.")]
    pub expires: String,

    #[property(description = "All domains (SAN DNS names + CN) the installed certificate is valid for.")]
    pub covered_domains: Vec<String>,

    #[property(description = "Endpoint domains that were checked against the certificate and matched.")]
    pub validated_endpoint_domains: Vec<String>,
}

pub struct InitSslCertificateHandler;

impl ToolDefinition for InitSslCertificateHandler {
    const FUNC_NAME: &'static str = "init_ssl_certificate";
    const DESCRIPTION: &'static str = "Upload (or replace) an SSL certificate in the running proxy by id. WORKFLOW: first call get_ssl_endpoints_status to find the endpoint whose certificate is missing or expired, note its `cert_id` and the domain (`server_name`) it protects, then obtain a PEM certificate + private key issued for that exact domain and pass them here. SAFETY: before storing, this validates that (1) the private key parses and is a supported type, (2) at least one endpoint references `cert_id`, and (3) the certificate actually covers every referencing endpoint's domain (SAN/CN match, wildcards honoured). If the certificate is for a different domain the upload is REFUSED — never bypass this by editing config. On success the certificate is served on the next TLS handshake with no reload. Note: the uploaded certificate is then manually managed and will not be auto-renewed from a configured source until RefreshSslCertificate is used.";
}

#[async_trait::async_trait]
impl McpToolCall<InitSslCertificateInputData, InitSslCertificateResponse>
    for InitSslCertificateHandler
{
    async fn execute_tool_call(
        &self,
        model: InitSslCertificateInputData,
    ) -> Result<InitSslCertificateResponse, String> {
        let result = crate::scripts::init_ssl_cert_manually(
            &model.cert_id,
            model.certificate.into_bytes(),
            model.private_key.into_bytes(),
        )
        .await?;

        Ok(InitSslCertificateResponse {
            cert_id: result.cert_id,
            cn: result.cn,
            expires: result.expires.to_rfc3339(),
            covered_domains: result.covered_domains,
            validated_endpoint_domains: result.validated_endpoint_domains,
        })
    }
}
