use std::sync::Arc;

use crate::{configurations::GoogleAuthCredentials, types::*};

pub enum GoogleAuthOkResult {
    Passed(Email),
    SetToken(Email),
    ShowLoginPage(Arc<GoogleAuthCredentials>),
}

#[derive(Debug)]
pub enum GoogleAuthError {
    ShowLogoutPage,
    // Content(hyper::Result<hyper::Response<BoxBody<Bytes, String>>>),
    EmailDomainIsNotAuthorized,
    ShowEmailDomainIsNotAuthorizedPage,
    ShowUserAuthenticatedPage(Email),

    ShowError(String),
}
