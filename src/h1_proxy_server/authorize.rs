use crate::{
    configurations::*, google_auth::GoogleAuthError, h1_utils::*,
    http_proxy_pass::HttpProxyPassIdentity, network_stream::*,
};

use super::*;

impl<TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static> H1ReadPart<TNetworkReadPart> {
    pub async fn authorize<'s>(
        &self,
        endpoint_info: &HttpEndpointInfo,
        http_connection_info: &HttpConnectionInfo,
        http_headers: &Http1Headers,
    ) -> Result<Option<HttpProxyPassIdentity>, ProxyServerError> {
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
                                    Http1ResponseBuilder::new_as_ok_result()
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
                                    Http1ResponseBuilder::new_as_ok_result()
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
                Http1ResponseBuilder::new_as_ok_result().build_with_body(body.as_bytes()),
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
                Http1ResponseBuilder::new_as_ok_result().build_with_body(body.as_bytes()),
            );
        }
        crate::google_auth::GoogleAuthError::ShowUserAuthenticatedPage(email) => {
            let body =
                crate::google_auth::generate_authenticated_user(headers_reader, email.as_str());
            return ProxyServerError::HttpResponse(
                Http1ResponseBuilder::new_as_ok_result().build_with_body(body.as_bytes()),
            );
        }
        crate::google_auth::GoogleAuthError::ShowError(err) => {
            return ProxyServerError::HttpResponse(
                Http1ResponseBuilder::new_as_ok_result().build_with_body(err.as_bytes()),
            );
        }
    }
}
