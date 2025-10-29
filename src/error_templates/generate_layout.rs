use rust_extensions::StrOrString;
use x509_parser::nom::AsBytes;

lazy_static::lazy_static! {
    pub static ref NOT_FOUND: Vec<u8> = {
       generate_layout(404, "Resource not found", None)
    };

    pub static ref REMOTE_RESOURCE_IS_NOT_AVAILABLE: Vec<u8> = {
       generate_layout(503, "Server Error", Some("Remote resource is not available".into()))
    };

    pub static ref LOCATION_IS_NOT_FOUND: Vec<u8> = {
       generate_layout(503, "Server Error", Some("Remote location configuration is missing".into()))
    };

    pub static ref CONFIGURATION_IS_NOT_FOUND: Vec<u8> = {
       generate_layout(503, "Server Error", Some("Endpoint configuration is missing".into()))
    };

     pub static ref ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE: Vec<u8> = {
       generate_layout(502, "Server Error", Some("Bad gateway".into()))
    };

    pub static ref NOT_AUTHORIZED_PAGE: Vec<u8> = {
       generate_layout(401, "Not authorized request", None)
    };
}

pub fn generate_layout(status_code: u16, text: &str, second_line: Option<StrOrString>) -> Vec<u8> {
    use crate::app::APP_VERSION;

    let second_line = if let Some(second_line) = second_line {
        format!("<h4>{}</h4>", second_line.as_str())
    } else {
        "".to_string()
    };

    let body = format!(
        r#"
        <div style="text-align: center;">
        <h2>{text}</h2>
      {second_line}
        <p>{status_code}</p>
        <hr/>
        <div>MyReverseProxy {APP_VERSION}</div>
        </div>
        "#
    )
    .into_bytes();

    let mut headers = crate::h1_utils::Http1HeadersBuilder::new();
    headers.add_response_first_line(200);

    headers.add_content_length(body.len());
    headers.write_cl_cr();

    let mut result = headers.into_bytes();
    result.extend_from_slice(body.as_bytes());
    result
}
