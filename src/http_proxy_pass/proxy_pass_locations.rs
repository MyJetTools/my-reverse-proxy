use hyper::Uri;

use crate::configurations::*;

use super::{ProxyPassError, ProxyPassLocation};

#[derive(Clone)]
pub struct LocationIndex {
    pub index: usize,
    pub id: i64,
}

pub struct ProxyPassLocations {
    data: Vec<ProxyPassLocation>,
}

impl ProxyPassLocations {
    pub async fn new(endpoint_info: &HttpEndpointInfo) -> Self {
        let mut data = Vec::with_capacity(endpoint_info.locations.len());
        for location in &endpoint_info.locations {
            let location = ProxyPassLocation::new(
                location.clone(),
                endpoint_info.debug,
                location.compress,
                location.trace_payload,
            )
            .await;
            data.push(location)
        }

        Self { data }
    }

    pub fn find_location_index(
        &self,
        uri: &Uri,
        debug: bool,
    ) -> Result<LocationIndex, ProxyPassError> {
        for (index, proxy_pass_location) in self.data.iter().enumerate() {
            if debug {
                println!(
                    "{} ProxyPass path: [{}] UriPath: [{}]",
                    index,
                    proxy_pass_location.config.path.as_str(),
                    uri.path()
                );
            }
            if proxy_pass_location.is_my_uri(uri) {
                if debug {
                    println!("Found location")
                }
                return Ok(LocationIndex {
                    index,
                    id: proxy_pass_location.config.id,
                });
            }
        }

        return Err(ProxyPassError::NoLocationFound);
    }

    pub fn find(&self, location_index: &LocationIndex) -> &ProxyPassLocation {
        if let Some(location) = self.data.get(location_index.index) {
            return location;
        }

        panic!(
            "find: Invalid location with id {} and index{}",
            location_index.id, location_index.index
        );
    }

    /*
    pub fn find_mut(&mut self, location_index: &LocationIndex) -> &mut ProxyPassLocation {
        if let Some(location) = self.data.get_mut(location_index.index) {
            return location;
        }

        panic!(
            "find_mut: Invalid location with id {} and index{}",
            location_index.id, location_index.index
        );
    }
     */
}
