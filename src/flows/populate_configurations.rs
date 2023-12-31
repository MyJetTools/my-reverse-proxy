use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    app::AppContext,
    http_server::{HttpProxyPassInner, ProxyPassError},
};

pub async fn populate_configurations(
    app: &AppContext,
    host: &str,
    inner: &mut Vec<HttpProxyPassInner>,
) -> Result<(), ProxyPassError> {
    let locations = app.settings_reader.get_configurations(host).await;

    if locations.len() == 0 {
        return Err(ProxyPassError::NoConfigurationFound);
    }

    let mut id = DateTimeAsMicroseconds::now().unix_microseconds;
    for (location, uri) in locations {
        inner.push(HttpProxyPassInner::new(location, uri, id));
        id += 1;
    }

    Ok(())
}
