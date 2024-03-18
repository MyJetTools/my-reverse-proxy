use hyper::Uri;

use crate::{
    app::AppContext,
    http_server::{ProxyPassConfiguration, ProxyPassError},
};

pub async fn find_proxy_pass_by_uri<'s>(
    app: &AppContext,
    inner: &'s mut Vec<ProxyPassConfiguration>,
    uri: &Uri,
) -> Result<&'s mut ProxyPassConfiguration, ProxyPassError> {
    let mut found_proxy_pass = None;
    for proxy_pass in inner.iter_mut() {
        if proxy_pass.is_my_uri(uri) {
            found_proxy_pass = Some(proxy_pass);
            break;
        }
    }

    if found_proxy_pass.is_none() {
        return Err(ProxyPassError::NoLocationFound);
    }

    let found_proxy_pass = found_proxy_pass.unwrap();

    found_proxy_pass.connect_if_require(app).await?;

    Ok(found_proxy_pass)
}
