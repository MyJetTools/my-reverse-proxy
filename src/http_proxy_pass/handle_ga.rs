use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full};

use crate::{
    app::AppContext,
    google_auth::{AUTHORIZED_PATH, LOGOUT_PATH},
    types::Email,
};

use super::{HttpProxyPass, HttpRequestBuilder, AUTHORIZED_COOKIE_NAME};

pub enum GoogleAuthResult {
    Passed(Option<Email>),
    Content(hyper::Result<hyper::Response<BoxBody<Bytes, String>>>),
    DomainIsNotAuthorized,
}

impl HttpProxyPass {
    pub(crate) async fn handle_auth_with_g_auth(
        &self,
        app: &AppContext,
        req: &HttpRequestBuilder,
    ) -> GoogleAuthResult {
        if self.endpoint_info.g_auth.is_none() {
            return GoogleAuthResult::Passed(None);
        }

        let g_auth_settings = self.endpoint_info.g_auth.as_ref().unwrap();

        let google_auth_credentials = app
            .current_configuration
            .get(|config| config.google_auth_credentials.get(g_auth_settings))
            .await;

        if google_auth_credentials.is_none() {
            panic!("Google Auth Credentials not found");
        }

        let google_auth_credentials = google_auth_credentials.unwrap();

        if req.uri().path() == LOGOUT_PATH {
            let body = Full::from(Bytes::from(
                crate::google_auth::generate_logout_page(req, "You have successfully logged out!")
                    .into_bytes(),
            ));

            let body = body.map_err(|e| crate::to_hyper_error(e)).boxed();

            return GoogleAuthResult::Content(Ok(hyper::Response::builder()
                .status(200)
                .body(body)
                .unwrap()));
        }

        if req.uri().path() == AUTHORIZED_PATH {
            if let Some(token) = req.get_authorization_token() {
                if let Some(email) = crate::google_auth::token::resolve(app, token) {
                    if !google_auth_credentials.domain_is_allowed(&email) {
                        let body = Full::from(Bytes::from(
                            crate::google_auth::generate_logout_page(
                                req,
                                "Unauthorized email domain",
                            )
                            .into_bytes(),
                        ));

                        return GoogleAuthResult::Content(Ok(hyper::Response::builder()
                            .status(200)
                            .body(body.map_err(|e| crate::to_hyper_error(e)).boxed())
                            .unwrap()));
                    }

                    let body = Full::from(Bytes::from(
                        crate::google_auth::generate_authorized_page(req, email.as_str())
                            .into_bytes(),
                    ));

                    return GoogleAuthResult::Content(Ok(hyper::Response::builder()
                        .status(200)
                        .body(body.map_err(|e| crate::to_hyper_error(e)).boxed())
                        .unwrap()));
                }
            }

            let code = req.get_from_query("code").unwrap();

            let email = match crate::google_auth::resolve_email(
                req,
                code.as_str(),
                &google_auth_credentials,
                self.endpoint_info.debug,
            )
            .await
            {
                Ok(email) => email,
                Err(err) => {
                    let body = Full::from(Bytes::from(err.into_bytes()));

                    return GoogleAuthResult::Content(Ok(hyper::Response::builder()
                        .status(400)
                        .body(body.map_err(|e| crate::to_hyper_error(e)).boxed())
                        .unwrap()));
                }
            };

            if !google_auth_credentials.domain_is_allowed(&email) {
                let body = Full::from(Bytes::from(
                    crate::google_auth::generate_logout_page(req, "Unauthorized email domain")
                        .into_bytes(),
                ));

                return GoogleAuthResult::Content(Ok(hyper::Response::builder()
                    .status(200)
                    .body(body.map_err(|e| crate::to_hyper_error(e)).boxed())
                    .unwrap()));
            }

            let body = Full::from(Bytes::from(
                crate::google_auth::generate_authorized_page(req, email.as_str()).into_bytes(),
            ));

            let token = crate::google_auth::token::generate(app, email.as_str());

            return GoogleAuthResult::Content(Ok(hyper::Response::builder()
                .status(200)
                .header(
                    "Set-Cookie",
                    format!("{}={}", AUTHORIZED_COOKIE_NAME, token),
                )
                .body(body.map_err(|e| crate::to_hyper_error(e)).boxed())
                .unwrap()));
        }

        if let Some(token) = req.get_authorization_token() {
            if let Some(email) = crate::google_auth::token::resolve(app, token) {
                if !google_auth_credentials.domain_is_allowed(&email) {
                    return GoogleAuthResult::DomainIsNotAuthorized;
                }
                return GoogleAuthResult::Passed(Some(email));
            }
        }

        let body = crate::google_auth::generate_login_page(req, &google_auth_credentials);

        let body = Full::from(Bytes::from(body.into_bytes()));

        return GoogleAuthResult::Content(Ok(hyper::Response::builder()
            .status(200)
            .body(body.map_err(|e| crate::to_hyper_error(e)).boxed())
            .unwrap()));
    }
}
