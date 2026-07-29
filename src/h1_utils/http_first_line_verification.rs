/// Verifies HTTP/1.1 first line format
/// Returns Ok(()) if valid, Err(String) with error description if invalid
pub fn verify_http11_first_line(first_line: &[u8]) -> Result<(), String> {
    if first_line.is_empty() {
        return Err("Empty first line".to_string());
    }

    // Convert to string for easier parsing
    let line_str = match std::str::from_utf8(first_line) {
        Ok(s) => s,
        Err(_) => return Err("Invalid UTF-8 in first line".to_string()),
    };

    let parts: Vec<&str> = line_str.split_whitespace().collect();

    if parts.is_empty() {
        return Err("First line contains no tokens".to_string());
    }

    // Check if it's a request line (METHOD URI HTTP/1.1) or response line (HTTP/1.1 STATUS REASON)
    if parts.len() < 2 {
        return Err("First line must contain at least 2 tokens".to_string());
    }

    // Determine if this is a request or response line
    let is_response = parts[0] == "HTTP/1.1";

    if is_response {
        // Response format: HTTP/1.1 STATUS REASON
        if parts.len() < 3 {
            return Err(
                "Response line must contain at least 3 tokens: HTTP/1.1 STATUS REASON".to_string(),
            );
        }

        // Validate status code
        let status_code = parts[1];
        if !is_valid_status_code(status_code) {
            return Err(format!("Invalid HTTP status code: '{}'", status_code));
        }
    } else {
        // Request format: METHOD URI HTTP/1.1
        if parts.len() != 3 {
            return Err(
                "Request line must contain exactly 3 tokens: METHOD URI HTTP/1.1".to_string(),
            );
        }

        // Validate HTTP version
        let http_version = parts[2];
        if http_version != "HTTP/1.1" {
            return Err(format!(
                "Invalid HTTP version: '{}', expected 'HTTP/1.1'",
                http_version
            ));
        }

        // Validate method
        let method = parts[0];
        if !is_valid_http_method(method) {
            return Err(format!("Invalid HTTP method: '{}'", method));
        }

        // Validate URI (basic check)
        let uri = parts[1];
        if uri.is_empty() {
            return Err("URI cannot be empty".to_string());
        }
    }

    Ok(())
}

/// Validates HTTP method
fn is_valid_http_method(method: &str) -> bool {
    matches!(
        method,
        "GET"
            | "POST"
            | "PUT"
            | "DELETE"
            | "HEAD"
            | "OPTIONS"
            | "PATCH"
            | "TRACE"
            | "CONNECT"
            | "PRI"
    )
}

/// Validates HTTP status code
fn is_valid_status_code(status: &str) -> bool {
    if status.len() != 3 {
        return false;
    }

    // Check if all characters are digits
    if !status.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // Check if status code is in valid range (100-599)
    if let Ok(code) = status.parse::<u16>() {
        return code >= 100 && code <= 599;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_http11_first_line_valid_requests() {
        // Valid request lines
        assert!(verify_http11_first_line(b"GET / HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"POST /api/users HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"PUT /users/123 HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"DELETE /items/456 HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"HEAD /status HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"OPTIONS /cors HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"PATCH /users/123 HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"TRACE /debug HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"CONNECT proxy.example.com:8080 HTTP/1.1").is_ok());
        assert!(verify_http11_first_line(b"PRI * HTTP/1.1").is_ok());
    }

    #[test]
    fn test_verify_http11_first_line_valid_responses() {
        // Valid response lines
        assert!(verify_http11_first_line(b"HTTP/1.1 200 OK").is_ok());
        assert!(verify_http11_first_line(b"HTTP/1.1 404 Not Found").is_ok());
        assert!(verify_http11_first_line(b"HTTP/1.1 500 Internal Server Error").is_ok());
        assert!(verify_http11_first_line(b"HTTP/1.1 301 Moved Permanently").is_ok());
        assert!(verify_http11_first_line(b"HTTP/1.1 100 Continue").is_ok());
        assert!(verify_http11_first_line(b"HTTP/1.1 599 Network Timeout").is_ok());
    }

    #[test]
    fn test_verify_http11_first_line_invalid_requests() {
        // Invalid request lines
        assert!(verify_http11_first_line(b"INVALID / HTTP/1.1").is_err());
        assert!(verify_http11_first_line(b"GET HTTP/1.1").is_err()); // Missing URI
        assert!(verify_http11_first_line(b"GET / HTTP/1.0").is_err()); // Wrong version
        assert!(verify_http11_first_line(b"GET / HTTP/2.0").is_err()); // Wrong version
        assert!(verify_http11_first_line(b"GET /").is_err()); // Missing version
        assert!(verify_http11_first_line(b"GET  HTTP/1.1").is_err()); // Empty URI
    }

    #[test]
    fn test_verify_http11_first_line_invalid_responses() {
        // Invalid response lines
        assert!(verify_http11_first_line(b"HTTP/1.1 999 Invalid").is_err()); // Invalid status code
        assert!(verify_http11_first_line(b"HTTP/1.1 99 Too Short").is_err()); // Invalid status code
        assert!(verify_http11_first_line(b"HTTP/1.1 abc Not Numeric").is_err()); // Non-numeric status
        assert!(verify_http11_first_line(b"HTTP/1.0 200 OK").is_err()); // Wrong version
        assert!(verify_http11_first_line(b"HTTP/2.0 200 OK").is_err()); // Wrong version
        assert!(verify_http11_first_line(b"HTTP/1.1 200").is_err()); // Missing reason phrase
    }

    #[test]
    fn test_verify_http11_first_line_edge_cases() {
        // Edge cases
        assert!(verify_http11_first_line(b"").is_err()); // Empty line
        assert!(verify_http11_first_line(b"   ").is_err()); // Only whitespace
        assert!(verify_http11_first_line(b"GET").is_err()); // Single token
        assert!(verify_http11_first_line(b"HTTP/1.1").is_err()); // Version only
    }

    #[test]
    fn test_verify_http11_first_line_utf8_errors() {
        // Test with invalid UTF-8
        let invalid_utf8 = b"GET /\xFF HTTP/1.1";
        assert!(verify_http11_first_line(invalid_utf8).is_err());
    }
}
