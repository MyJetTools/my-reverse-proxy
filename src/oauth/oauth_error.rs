use serde::Serialize;

use super::OAuthHttpResponse;

/// The RFC 6749 §5.2 error body. Claude matches on `error`, so the codes have to
/// be the registered ones — a custom string reads as an unknown failure and the
/// connector gives up instead of retrying the flow.
#[derive(Serialize)]
struct OAuthErrorBody<'s> {
    error: &'s str,
    error_description: &'s str,
}

pub const ERROR_INVALID_REQUEST: &str = "invalid_request";
pub const ERROR_INVALID_CLIENT: &str = "invalid_client";
pub const ERROR_INVALID_GRANT: &str = "invalid_grant";
pub const ERROR_UNSUPPORTED_GRANT_TYPE: &str = "unsupported_grant_type";
pub const ERROR_UNSUPPORTED_RESPONSE_TYPE: &str = "unsupported_response_type";
pub const ERROR_INVALID_TOKEN: &str = "invalid_token";
pub const ERROR_SERVER_ERROR: &str = "server_error";

pub fn oauth_error_response(
    status_code: u16,
    error: &str,
    error_description: &str,
) -> OAuthHttpResponse {
    OAuthHttpResponse::json(status_code, error_body(error, error_description))
}

pub fn error_body(error: &str, error_description: &str) -> Vec<u8> {
    let body = OAuthErrorBody {
        error,
        error_description,
    };

    // The struct is two borrowed strings — serialisation can not fail, but a
    // panic in a request handler must not be on the table either.
    serde_json::to_vec(&body).unwrap_or_else(|_| {
        br#"{"error":"server_error","error_description":"Can not build the error body"}"#.to_vec()
    })
}

pub fn method_not_allowed(allowed: &str) -> OAuthHttpResponse {
    oauth_error_response(
        405,
        ERROR_INVALID_REQUEST,
        &format!("This endpoint only accepts {}", allowed),
    )
    .add_header("Allow", allowed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_body_is_the_rfc_6749_shape() {
        let body = error_body(ERROR_INVALID_GRANT, "The authorization code has expired");

        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(parsed["error"], "invalid_grant");
        assert_eq!(
            parsed["error_description"],
            "The authorization code has expired"
        );
    }

    #[test]
    fn method_not_allowed_advertises_what_is_allowed() {
        let response = method_not_allowed("POST");

        assert_eq!(response.status_code, 405);
        assert!(response
            .headers
            .iter()
            .any(|header| header.name == "Allow" && header.value == "POST"));
    }
}
