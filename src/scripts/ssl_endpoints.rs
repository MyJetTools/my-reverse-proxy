use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::configurations::{ListenConfiguration, SslCertificateIdRef};

/// Certificate that is within this many days of expiry is reported as `ExpiringSoon`
/// (mirrors the renewal timer threshold).
const EXPIRING_SOON_DAYS: i64 = 7;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SslCertStatus {
    /// Referenced by an endpoint but not present in the running cache — the TLS
    /// handshake for this endpoint currently fails.
    Missing,
    /// Present but already past its `not_after`.
    Expired,
    /// Present and valid, but expires within `EXPIRING_SOON_DAYS` days.
    ExpiringSoon,
    Ok,
}

impl SslCertStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SslCertStatus::Missing => "missing",
            SslCertStatus::Expired => "expired",
            SslCertStatus::ExpiringSoon => "expiring_soon",
            SslCertStatus::Ok => "ok",
        }
    }

    /// True when the certificate is absent or expired — i.e. the endpoint needs a
    /// new certificate to serve TLS.
    pub fn needs_attention(&self) -> bool {
        matches!(self, SslCertStatus::Missing | SslCertStatus::Expired)
    }
}

pub struct SslEndpointCertStatus {
    /// Full endpoint host as configured, e.g. `example.com:443` or a unix socket path.
    pub endpoint_host: String,
    /// The domain (SNI server name) the endpoint protects, if any. `None` for a
    /// default endpoint that has no server name.
    pub server_name: Option<String>,
    pub cert_id: String,
    pub status: SslCertStatus,
    pub cn: Option<String>,
    pub expires: Option<DateTimeAsMicroseconds>,
    pub days_to_expire: Option<i64>,
}

/// Enumerates every https/mcp endpoint that references a real (non self-signed) SSL
/// certificate and reports whether that certificate is present, expired or expiring soon.
pub async fn get_ssl_endpoints_status() -> Vec<SslEndpointCertStatus> {
    // Collect (endpoint_host, server_name, cert_id) for every https/mcp endpoint that
    // points at a non self-signed certificate, under a single config read.
    let endpoints: Vec<(String, Option<String>, String)> = crate::app::APP_CTX
        .current_configuration
        .get(|cfg| {
            let mut out = Vec::new();
            for listen in cfg.listen_tcp_endpoints.values() {
                collect_endpoints(listen, &mut out);
            }
            for listen in cfg.listen_unix_socket_endpoints.values() {
                collect_endpoints(listen, &mut out);
            }
            out
        })
        .await;

    let now = DateTimeAsMicroseconds::now();

    let mut result = Vec::with_capacity(endpoints.len());

    for (endpoint_host, server_name, cert_id) in endpoints {
        let holder = crate::app::APP_CTX
            .ssl_certificates_cache
            .read(|c| c.ssl_certs.get(SslCertificateIdRef::new(cert_id.as_str())))
            .await;

        let (status, cn, expires, days_to_expire) = match holder {
            None => (SslCertStatus::Missing, None, None, None),
            Some(holder) => {
                let info = holder.ssl_cert.get_cert_info();
                let days = info.expires.duration_since(now).get_full_days();
                let status = if info.expires.unix_microseconds <= now.unix_microseconds {
                    SslCertStatus::Expired
                } else if days <= EXPIRING_SOON_DAYS {
                    SslCertStatus::ExpiringSoon
                } else {
                    SslCertStatus::Ok
                };
                (status, Some(info.cn), Some(info.expires), Some(days))
            }
        };

        result.push(SslEndpointCertStatus {
            endpoint_host,
            server_name,
            cert_id,
            status,
            cn,
            expires,
            days_to_expire,
        });
    }

    result
}

fn collect_endpoints(
    listen: &ListenConfiguration,
    out: &mut Vec<(String, Option<String>, String)>,
) {
    let http = match listen {
        ListenConfiguration::Http(http) | ListenConfiguration::Mcp(http) => http,
        ListenConfiguration::Tcp(_) => return,
    };

    for endpoint in http.endpoints.iter() {
        let Some(ssl_cert_id) = endpoint.ssl_certificate_id.as_ref() else {
            continue;
        };

        if ssl_cert_id.is_self_signed() {
            continue;
        }

        out.push((
            endpoint.as_str().to_string(),
            endpoint
                .host_endpoint
                .get_server_name()
                .map(|s| s.to_string()),
            ssl_cert_id.as_str().to_string(),
        ));
    }
}
