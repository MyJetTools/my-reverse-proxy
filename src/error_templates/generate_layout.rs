use rust_extensions::StrOrString;

lazy_static::lazy_static! {
    pub static ref NOT_FOUND: Vec<u8> = {
       generate_layout(404, "Resource not found", None)
    };
}

pub fn generate_layout(status_code: u16, text: &str, second_line: Option<StrOrString>) -> Vec<u8> {
    use crate::app::APP_VERSION;

    let second_line = if let Some(second_line) = second_line {
        format!("<h4>{}</h4>", second_line.as_str())
    } else {
        "".to_string()
    };

    format!(
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
    .into_bytes()
}
