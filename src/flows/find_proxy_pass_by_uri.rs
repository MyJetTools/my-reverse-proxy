use hyper::Uri;

use crate::{
    http_client::{HttpClient, HttpClientConnection},
    http_server::{HttpProxyPassInner, ProxyPassError},
};

pub async fn find_proxy_pass_by_uri<'s>(
    inner: &'s mut Vec<HttpProxyPassInner>,
    uri: &Uri,
) -> Result<&'s mut HttpProxyPassInner, ProxyPassError> {
    let mut found_proxy_pass = None;
    for proxy_pass in inner.iter_mut() {
        if proxy_pass.is_my_uri(uri) {
            println!(
                "{} goes to {}",
                uri.path(),
                proxy_pass.proxy_pass_uri.to_string()
            );

            found_proxy_pass = Some(proxy_pass);
            break;
        }
    }

    if found_proxy_pass.is_none() {
        return Err(ProxyPassError::NoLocationFound);
    }

    let found_proxy_pass = found_proxy_pass.unwrap();

    if found_proxy_pass.http_client.connection.is_none() {
        let connection = HttpClient::connect(&found_proxy_pass.proxy_pass_uri).await?;
        found_proxy_pass.http_client.connection = Some(HttpClientConnection::new(connection));
    }

    Ok(found_proxy_pass)
}
