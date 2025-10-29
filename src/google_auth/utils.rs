use crate::types::*;

pub fn generate_redirect_url(req: &impl HttpRequestReader) -> String {
    format!(
        "{}{}",
        req.get_host().unwrap_or_default(),
        super::AUTHORIZED_PATH
    )
}
