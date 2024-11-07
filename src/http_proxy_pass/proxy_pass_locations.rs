use hyper::Uri;

use crate::{app::AppContext, configurations::*};

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
    pub fn new(app: &AppContext, endpoint_info: &HttpEndpointInfo) -> Self {
        let mut data = Vec::with_capacity(endpoint_info.locations.len());
        for location in &endpoint_info.locations {
            let location = ProxyPassLocation::new(
                app,
                location.clone(),
                endpoint_info.debug,
                location.compress,
            );
            data.push(location)
        }

        Self { data }
    }

    pub fn find_location_index(&self, uri: &Uri) -> Result<LocationIndex, ProxyPassError> {
        for (index, proxy_pass) in self.data.iter().enumerate() {
            if proxy_pass.is_my_uri(uri) {
                return Ok(LocationIndex {
                    index,
                    id: proxy_pass.config.id,
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
