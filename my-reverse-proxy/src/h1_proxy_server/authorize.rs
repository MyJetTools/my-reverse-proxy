use crate::{
    configurations::*, google_auth::GoogleAuthError, h1_utils::*,
    http_proxy_pass::HttpProxyPassIdentity, network_stream::*,
};

use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use super::*;

const AUTHORIZATION_HEADER: &[u8] = b"authorization";

fn find_authorization_value<'a>(
    http_headers: &Http1Headers,
    data: &'a [u8],
) -> Option<&'a [u8]> {
    let mut pos = http_headers.first_line_end + crate::consts::HTTP_CR_LF.len();
    loop {
        let next = data.find_sequence_pos(crate::consts::HTTP_CR_LF, pos)?;
        if next == pos {
            return None;
        }
        if let Some(header) = HttpHeader::new(data, pos, next) {
            if header.is_my_header_name(AUTHORIZATION_HEADER) {
                let value_pos = header.get_value();
                let raw = &data[value_pos.start..value_pos.end];
                let trimmed = trim_ascii(raw);
                return Some(trimmed);
            }
        }
        pos = next + crate::consts::HTTP_CR_LF.len();
    }
}

fn trim_ascii(input: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = input.len();
    while start < end && (input[start] == b' ' || input[start] == b'\t') {
        start += 1;
    }
    while end > start && (input[end - 1] == b' ' || input[end - 1] == b'\t') {
        end -= 1;
    }
    &input[start..end]
}

impl<TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static> H1Reader<TNetworkReadPart> {
    pub async fn authorize(
        &self,
        endpoint_info: &HttpEndpointInfo,
        location: &ProxyPassLocationConfig,
        http_connection_info: &HttpConnectionInfo,
        http_headers: &Http1Headers,
    ) -> Result<Option<HttpProxyPassIdentity>, ProxyServerError> {
        if let Some(expected) = location.auth_header.as_deref() {
            let actual = find_authorization_value(http_headers, self.loop_buffer.get_data());
            let matches = match actual {
                Some(actual) => actual == expected.as_bytes(),
                None => false,
            };
            if !matches {
                return Err(ProxyServerError::NotAuthorized);
            }
        }

        if let Some(authorize) = endpoint_info.must_be_authorized() {
            match authorize {
                AuthorizationRequired::GoogleAuth(google_auth_settings) => {
                    let headers_reader = HttpHeadersReader {
                        http_headers,
                        payload: self.loop_buffer.get_data(),
                    };
                    let result = crate::google_auth::handle_google_auth(
                        &headers_reader,
                        google_auth_settings,
                        endpoint_info.debug,
                    )
                    .await;

                    match result {
                        Ok(ok_result) => match ok_result {
                            crate::google_auth::GoogleAuthOkResult::Passed(email) => {
                                return Ok(Some(HttpProxyPassIdentity::GoogleUser(email)));
                            }
                            crate::google_auth::GoogleAuthOkResult::SetToken(email) => {
                                let body = crate::google_auth::generate_authenticated_user(
                                    &headers_reader,
                                    email.as_str(),
                                );
                                let token = crate::google_auth::token::generate(email.as_str());
                                let cookie_value = format!(
                                    "{}={}; SameSite=None; Secure;",
                                    crate::consts::AUTHORIZED_COOKIE_NAME,
                                    token
                                );
                                return Err(ProxyServerError::HttpResponse(
                                    Http1ResponseBuilder::new_as_html()
                                        .add_header("Set-Cookie", cookie_value.as_str())
                                        .build_with_body(body.as_bytes()),
                                ));
                            }
                            crate::google_auth::GoogleAuthOkResult::ShowLoginPage(
                                google_auth_credentials,
                            ) => {
                                let body = crate::google_auth::generate_login_page(
                                    &headers_reader,
                                    &google_auth_credentials,
                                );
                                return Err(ProxyServerError::HttpResponse(
                                    Http1ResponseBuilder::new_as_html()
                                        .build_with_body(body.as_bytes()),
                                ));
                            }
                        },
                        Err(err) => {
                            let err = from_google_auth_error(err, &headers_reader);
                            return Err(err);
                        }
                    }
                }
                AuthorizationRequired::ClientCertificate => {
                    return Ok(http_connection_info
                        .cn_user_name
                        .clone()
                        .map(|cert| HttpProxyPassIdentity::ClientCert(cert)));
                }
            };
        }

        return Ok(http_connection_info
            .cn_user_name
            .clone()
            .map(|cert| HttpProxyPassIdentity::ClientCert(cert)));
    }
}

fn from_google_auth_error(
    value: GoogleAuthError,
    headers_reader: &impl crate::types::HttpRequestReader,
) -> ProxyServerError {
    match value {
        crate::google_auth::GoogleAuthError::ShowLogoutPage => {
            let body = crate::google_auth::generate_logout_page(
                headers_reader,
                "You have successfully logged out!",
            );
            return ProxyServerError::HttpResponse(
                Http1ResponseBuilder::new_as_html().build_with_body(body.as_bytes()),
            );
        }
        crate::google_auth::GoogleAuthError::EmailDomainIsNotAuthorized => {
            return ProxyServerError::NotAuthorized;
        }
        crate::google_auth::GoogleAuthError::ShowEmailDomainIsNotAuthorizedPage => {
            let body = crate::google_auth::generate_logout_page(
                headers_reader,
                "Unauthorized email domain",
            );
            return ProxyServerError::HttpResponse(
                Http1ResponseBuilder::new_as_html().build_with_body(body.as_bytes()),
            );
        }
        crate::google_auth::GoogleAuthError::ShowUserAuthenticatedPage(email) => {
            let body =
                crate::google_auth::generate_authenticated_user(headers_reader, email.as_str());
            return ProxyServerError::HttpResponse(
                Http1ResponseBuilder::new_as_html().build_with_body(body.as_bytes()),
            );
        }
        crate::google_auth::GoogleAuthError::ShowError(err) => {
            return ProxyServerError::HttpResponse(
                Http1ResponseBuilder::new_as_html().build_with_body(err.as_bytes()),
            );
        }
    }
}
