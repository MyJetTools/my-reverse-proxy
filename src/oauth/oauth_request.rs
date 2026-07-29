use super::OAuthRoute;

/// Only the two methods the OAuth endpoints use are named; anything else is
/// answered with 405 rather than silently treated as a GET.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthMethod {
    Get,
    Post,
    Other,
}

impl OAuthMethod {
    pub fn parse(method: &str) -> Self {
        if method.eq_ignore_ascii_case("GET") {
            return Self::Get;
        }

        if method.eq_ignore_ascii_case("POST") {
            return Self::Post;
        }

        Self::Other
    }
}

/// Everything the OAuth core needs about an inbound request, lifted out of
/// whichever pipeline read it. Deliberately holds no transport type, so the h1
/// byte reader and the hyper-based h2 handler feed the same code.
pub struct OAuthRequest<'s> {
    pub method: OAuthMethod,
    pub route: OAuthRoute,
    /// Scheme + host of this endpoint, no trailing slash — the issuer.
    pub base_url: &'s str,
    /// Raw query string, without the leading `?`.
    pub query: Option<&'s str>,
    pub body: &'s [u8],
    pub content_type: Option<&'s str>,
    /// Raw `Authorization` header value, used for HTTP Basic client
    /// authentication on the token endpoint.
    pub authorization: Option<&'s str>,
    /// Paths of the locations configured on this endpoint — what the protected
    /// resource metadata is allowed to describe.
    pub known_resource_paths: &'s [&'s str],
}

impl OAuthRequest<'_> {
    /// The parameters of this request, from the query string on a GET and from
    /// the form-urlencoded body on a POST.
    ///
    /// `/authorize` is specified to take its parameters in the query string and
    /// the consent form posts them back in the body, so the same handler reads
    /// whichever one carries them.
    pub fn parameters(&self) -> std::collections::HashMap<String, String> {
        match self.method {
            OAuthMethod::Post => super::parse_form_urlencoded(&String::from_utf8_lossy(self.body)),
            OAuthMethod::Get | OAuthMethod::Other => {
                super::parse_form_urlencoded(self.query.unwrap_or_default())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'s>(
        method: OAuthMethod,
        query: Option<&'s str>,
        body: &'s [u8],
    ) -> OAuthRequest<'s> {
        OAuthRequest {
            method,
            route: OAuthRoute::Authorize,
            base_url: "https://mcp-home.jetdev.eu",
            query,
            body,
            content_type: None,
            authorization: None,
            known_resource_paths: &[],
        }
    }

    #[test]
    fn methods_parse_case_insensitively() {
        assert_eq!(OAuthMethod::parse("get"), OAuthMethod::Get);
        assert_eq!(OAuthMethod::parse("POST"), OAuthMethod::Post);
        assert_eq!(OAuthMethod::parse("DELETE"), OAuthMethod::Other);
    }

    #[test]
    fn a_get_reads_its_parameters_from_the_query_string() {
        let request = request(OAuthMethod::Get, Some("client_id=claude&state=xyz"), b"");

        let parameters = request.parameters();

        assert_eq!(parameters.get("client_id").unwrap(), "claude");
        assert_eq!(parameters.get("state").unwrap(), "xyz");
    }

    #[test]
    fn a_post_reads_its_parameters_from_the_body() {
        let request = request(OAuthMethod::Post, Some("ignored=yes"), b"client_id=claude");

        let parameters = request.parameters();

        assert_eq!(parameters.get("client_id").unwrap(), "claude");
        assert!(!parameters.contains_key("ignored"));
    }
}
