use crate::configurations::*;

use crate::types::*;

pub fn generate_login_page(
    req: &impl HttpRequestReader,
    settings: &GoogleAuthCredentials,
) -> String {
    return super::html::generate_with_template(|| {
        format!(
            r###"<a class="btn btn-primary" href="https://accounts.google.com/o/oauth2/v2/auth?scope=openid%20email&access_type=offline&include_granted_scopes=true&response_type=code&state=state_parameter_passthrough_value&redirect_uri=https%3A//{}&client_id={}">Sign in with Google</a>"###,
            super::generate_redirect_url(req),
            settings.client_id.as_str()
        )
    });
}
