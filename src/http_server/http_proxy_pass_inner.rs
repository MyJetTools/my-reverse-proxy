use hyper::Uri;

use crate::http_client::HttpClient;

pub struct HttpProxyPassInner {
    pub http_client: HttpClient,
    pub proxy_pass_uri: Uri,
    pub location: String,
    pub id: i64,
}

impl HttpProxyPassInner {
    pub fn new(location: String, proxy_pass_uri: Uri, id: i64) -> Self {
        Self {
            location,
            http_client: HttpClient::new(),
            proxy_pass_uri,
            id,
        }
    }

    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.location.as_str(),
        );

        result
    }
}
