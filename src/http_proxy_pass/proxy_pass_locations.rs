use hyper::Uri;

use super::{ProxyPassError, ProxyPassLocation};

pub struct LocationIndex {
    pub index: usize,
    pub id: i64,
}

pub struct ProxyPassLocations {
    data: Option<Vec<ProxyPassLocation>>,
}

impl ProxyPassLocations {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn init(&mut self, locations: Vec<ProxyPassLocation>) {
        self.data = Some(locations);
    }

    pub fn find_location_index(&self, uri: &Uri) -> Result<LocationIndex, ProxyPassError> {
        for (index, proxy_pass) in self.data.as_ref().unwrap().iter().enumerate() {
            if proxy_pass.is_my_uri(uri) {
                return Ok(LocationIndex {
                    index,
                    id: proxy_pass.id,
                });
            }
        }

        return Err(ProxyPassError::NoLocationFound);
    }

    pub fn find(&self, location_index: &LocationIndex) -> &ProxyPassLocation {
        if let Some(locations) = self.data.as_ref() {
            if let Some(location) = locations.get(location_index.index) {
                return location;
            }

            panic!(
                "find: Invalid location with id {} and index{}",
                location_index.id, location_index.index
            );
        }

        panic!("find: Locations are not initialized")
    }

    pub fn find_mut(&mut self, location_index: &LocationIndex) -> &mut ProxyPassLocation {
        if let Some(locations) = self.data.as_mut() {
            if let Some(location) = locations.get_mut(location_index.index) {
                return location;
            }

            panic!(
                "find_mut: Invalid location with id {} and index{}",
                location_index.id, location_index.index
            );
        }

        panic!("find_mut: Locations are not initialized")
    }

    pub fn has_configurations(&self) -> bool {
        self.data.is_some()
    }
}
