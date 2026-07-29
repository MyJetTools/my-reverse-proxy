/// One response header the OAuth core wants set. Named rather than a `(String,
/// String)` so the two sides can not be swapped at a call site.
pub struct OAuthHeader {
    pub name: String,
    pub value: String,
}

/// A complete response produced by the OAuth core, in a shape both the h1 and
/// the h2 pipelines can render. The core never touches a socket — this is the
/// whole of its output.
pub struct OAuthHttpResponse {
    pub status_code: u16,
    pub content_type: &'static str,
    pub headers: Vec<OAuthHeader>,
    pub body: Vec<u8>,
    /// Set when this response is the answer to a credential guess (a wrong
    /// consent password or client secret). The transport registers it against
    /// the source IP so the existing block-list can throttle brute force.
    pub register_ip_failure: bool,
}

pub const CONTENT_TYPE_JSON: &str = "application/json";
pub const CONTENT_TYPE_HTML: &str = "text/html; charset=UTF-8";

impl OAuthHttpResponse {
    pub fn json(status_code: u16, body: Vec<u8>) -> Self {
        Self {
            status_code,
            content_type: CONTENT_TYPE_JSON,
            // OAuth responses carry credentials — never let anything cache them.
            headers: vec![
                OAuthHeader {
                    name: "Cache-Control".to_string(),
                    value: "no-store".to_string(),
                },
                OAuthHeader {
                    name: "Pragma".to_string(),
                    value: "no-cache".to_string(),
                },
            ],
            body,
            register_ip_failure: false,
        }
    }

    /// Metadata documents are public and stable, so they may be cached — and
    /// Claude re-fetches them on every connection attempt.
    pub fn metadata(body: Vec<u8>) -> Self {
        Self {
            status_code: 200,
            content_type: CONTENT_TYPE_JSON,
            headers: vec![OAuthHeader {
                name: "Cache-Control".to_string(),
                value: "max-age=300".to_string(),
            }],
            body,
            register_ip_failure: false,
        }
    }

    pub fn html(status_code: u16, body: String) -> Self {
        Self {
            status_code,
            content_type: CONTENT_TYPE_HTML,
            headers: vec![OAuthHeader {
                name: "Cache-Control".to_string(),
                value: "no-store".to_string(),
            }],
            body: body.into_bytes(),
            register_ip_failure: false,
        }
    }

    pub fn redirect(location: String) -> Self {
        Self {
            status_code: 302,
            content_type: CONTENT_TYPE_HTML,
            headers: vec![
                OAuthHeader {
                    name: "Location".to_string(),
                    value: location,
                },
                OAuthHeader {
                    name: "Cache-Control".to_string(),
                    value: "no-store".to_string(),
                },
            ],
            body: Vec::new(),
            register_ip_failure: false,
        }
    }

    pub fn add_header(mut self, name: &str, value: String) -> Self {
        self.headers.push(OAuthHeader {
            name: name.to_string(),
            value,
        });
        self
    }

    pub fn into_credential_failure(mut self) -> Self {
        self.register_ip_failure = true;
        self
    }
}
