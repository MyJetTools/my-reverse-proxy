use hyper::Uri;

use super::{ProxyPassConfiguration, ProxyPassError};

pub struct ProxyPassConfigurations {
    data: Option<Vec<ProxyPassConfiguration>>,
}

impl ProxyPassConfigurations {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn init(&mut self, configurations: Vec<ProxyPassConfiguration>) {
        self.data = Some(configurations);
    }

    pub fn find(&mut self, uri: &Uri) -> Result<&mut ProxyPassConfiguration, ProxyPassError> {
        let mut found_proxy_pass = None;
        for proxy_pass in self.data.as_mut().unwrap() {
            if proxy_pass.is_my_uri(uri) {
                found_proxy_pass = Some(proxy_pass);
                break;
            }
        }

        if found_proxy_pass.is_none() {
            return Err(ProxyPassError::NoLocationFound);
        }

        //let found_proxy_pass = found_proxy_pass.unwrap();

        // found_proxy_pass.connect_if_require(app).await?;

        Ok(found_proxy_pass.unwrap())
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<ProxyPassConfiguration> {
        self.data.as_mut().unwrap().iter_mut()
    }

    pub fn has_configurations(&self) -> bool {
        self.data.is_some()
    }
}
