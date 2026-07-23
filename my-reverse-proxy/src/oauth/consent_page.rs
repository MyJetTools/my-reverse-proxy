use super::{html_escape, AUTHORIZE_PATH};

/// Everything the consent form has to carry back to the POST, plus what the
/// person looking at it needs in order to decide.
pub struct ConsentPageParams<'s> {
    pub client_id: &'s str,
    pub redirect_uri: &'s str,
    pub state: Option<&'s str>,
    pub scope: &'s str,
    pub code_challenge: &'s str,
    pub resource: Option<&'s str>,
    /// Shown when the previous attempt was refused.
    pub error: Option<&'s str>,
}

/// The one interactive screen in the flow: the user proves they are allowed to
/// hand this connector access by typing the block's `consent_password`.
///
/// Modelled on the google_auth login page — a self-contained document with no
/// external stylesheet, so it renders the same behind any egress policy.
pub fn generate_consent_page(params: &ConsentPageParams) -> String {
    let error_block = match params.error {
        Some(error) => format!(r#"<p class="error">{}</p>"#, html_escape(error)),
        None => String::new(),
    };

    let resource_row = match params.resource {
        Some(resource) => format!(
            r#"<tr><th>Resource</th><td>{}</td></tr>"#,
            html_escape(resource)
        ),
        None => String::new(),
    };

    let state_field = match params.state {
        Some(state) => hidden_field("state", state),
        None => String::new(),
    };

    let resource_field = match params.resource {
        Some(resource) => hidden_field("resource", resource),
        None => String::new(),
    };

    format!(
        r###"<html><head><title>Authorize connector</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
body {{ font-family: -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
       background: #f5f6f8; margin: 0; padding: 0; color: #1c1e21; }}
#card {{ max-width: 460px; margin: 8vh auto; background: #fff; border-radius: 10px;
        box-shadow: 0 1px 4px rgba(0,0,0,.14); padding: 28px 32px; }}
h1 {{ font-size: 20px; margin: 0 0 6px 0; }}
p.lead {{ color: #55595e; font-size: 14px; margin: 0 0 20px 0; }}
table {{ width: 100%; border-collapse: collapse; margin-bottom: 20px; font-size: 13px; }}
th {{ text-align: left; color: #6a6e73; font-weight: 500; width: 34%; padding: 5px 0; vertical-align: top; }}
td {{ padding: 5px 0; word-break: break-all; }}
label {{ display: block; font-size: 13px; margin-bottom: 6px; color: #37393d; }}
input[type=password] {{ width: 100%; box-sizing: border-box; padding: 9px 11px; font-size: 14px;
                        border: 1px solid #ccd0d5; border-radius: 6px; }}
button {{ margin-top: 16px; width: 100%; padding: 10px; font-size: 15px; border: 0;
          border-radius: 6px; background: #2f6fdb; color: #fff; cursor: pointer; }}
button:hover {{ background: #2a61c0; }}
p.error {{ background: #fdecec; border: 1px solid #f3c2c2; color: #a12020;
          padding: 9px 11px; border-radius: 6px; font-size: 13px; margin: 0 0 16px 0; }}
</style>
</head><body>
<div id="card">
<h1>Authorize connector</h1>
<p class="lead">An application is asking for access through this proxy. Enter the consent password to approve it.</p>
{error_block}
<table>
<tr><th>Client</th><td>{client_id}</td></tr>
{resource_row}
<tr><th>Scope</th><td>{scope}</td></tr>
<tr><th>Redirect</th><td>{redirect_uri}</td></tr>
</table>
<form method="POST" action="{authorize_path}">
{hidden_client_id}
{hidden_redirect_uri}
{hidden_response_type}
{hidden_scope}
{hidden_code_challenge}
{hidden_code_challenge_method}
{state_field}
{resource_field}
<label for="consent_password">Consent password</label>
<input type="password" id="consent_password" name="consent_password" autocomplete="off" autofocus spellcheck="false">
<button type="submit">Approve</button>
</form>
</div>
</body></html>"###,
        error_block = error_block,
        client_id = html_escape(params.client_id),
        resource_row = resource_row,
        scope = html_escape(params.scope),
        redirect_uri = html_escape(params.redirect_uri),
        authorize_path = AUTHORIZE_PATH,
        hidden_client_id = hidden_field("client_id", params.client_id),
        hidden_redirect_uri = hidden_field("redirect_uri", params.redirect_uri),
        hidden_response_type = hidden_field("response_type", "code"),
        hidden_scope = hidden_field("scope", params.scope),
        hidden_code_challenge = hidden_field("code_challenge", params.code_challenge),
        hidden_code_challenge_method = hidden_field("code_challenge_method", "S256"),
        state_field = state_field,
        resource_field = resource_field,
    )
}

/// Shown when the request can not be sent back to the client — an unknown
/// `client_id` or a `redirect_uri` this server will not honour. Redirecting
/// those would turn the authorize endpoint into an open redirector, so the
/// message stops here instead.
pub fn generate_error_page(title: &str, message: &str) -> String {
    format!(
        r###"<html><head><title>{title}</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
body {{ font-family: -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
       background: #f5f6f8; margin: 0; color: #1c1e21; }}
#card {{ max-width: 460px; margin: 8vh auto; background: #fff; border-radius: 10px;
        box-shadow: 0 1px 4px rgba(0,0,0,.14); padding: 28px 32px; }}
h1 {{ font-size: 20px; margin: 0 0 10px 0; }}
p {{ color: #55595e; font-size: 14px; margin: 0; }}
</style>
</head><body><div id="card"><h1>{title}</h1><p>{message}</p></div></body></html>"###,
        title = html_escape(title),
        message = html_escape(message),
    )
}

fn hidden_field(name: &str, value: &str) -> String {
    format!(
        r#"<input type="hidden" name="{}" value="{}">"#,
        html_escape(name),
        html_escape(value)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params<'s>() -> ConsentPageParams<'s> {
        ConsentPageParams {
            client_id: "claude",
            redirect_uri: "https://claude.ai/api/mcp/auth_callback",
            state: Some("the-state"),
            scope: "mcp offline_access",
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            resource: Some("https://mcp-home.jetdev.eu/mt-risks"),
            error: None,
        }
    }

    #[test]
    fn every_parameter_needed_to_finish_the_flow_round_trips_in_the_form() {
        let page = generate_consent_page(&params());

        for expected in [
            r#"name="client_id" value="claude""#,
            r#"name="redirect_uri" value="https://claude.ai/api/mcp/auth_callback""#,
            r#"name="state" value="the-state""#,
            r#"name="scope" value="mcp offline_access""#,
            r#"name="code_challenge" value="E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM""#,
            r#"name="code_challenge_method" value="S256""#,
            r#"name="resource" value="https://mcp-home.jetdev.eu/mt-risks""#,
            r#"name="consent_password""#,
        ] {
            assert!(
                page.contains(expected),
                "missing from the form: {}",
                expected
            );
        }
    }

    #[test]
    fn a_missing_state_leaves_the_field_out_rather_than_sending_an_empty_one() {
        let page = generate_consent_page(&ConsentPageParams {
            state: None,
            resource: None,
            ..params()
        });

        assert!(!page.contains(r#"name="state""#));
        assert!(!page.contains(r#"name="resource""#));
    }

    #[test]
    fn values_can_not_break_out_of_the_markup() {
        let page = generate_consent_page(&ConsentPageParams {
            client_id: r#"claude"><script>alert(1)</script>"#,
            ..params()
        });

        assert!(!page.contains("<script>alert(1)</script>"));
        assert!(page.contains("&lt;script&gt;"));
    }

    #[test]
    fn the_error_is_shown_when_a_password_was_refused() {
        let page = generate_consent_page(&ConsentPageParams {
            error: Some("Wrong consent password"),
            ..params()
        });

        assert!(page.contains("Wrong consent password"));
    }

    #[test]
    fn the_error_page_escapes_what_it_is_given() {
        let page = generate_error_page("Invalid request", "<b>redirect_uri</b> is not allowed");

        assert!(page.contains("&lt;b&gt;redirect_uri&lt;/b&gt;"));
        assert!(!page.contains("<b>redirect_uri</b>"));
    }
}
