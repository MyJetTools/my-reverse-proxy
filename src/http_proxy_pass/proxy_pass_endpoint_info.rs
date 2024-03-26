use super::HttpType;

#[derive(Debug)]
pub struct ProxyPassEndpointInfo {
    pub host_endpoint: String,
    pub debug: bool,
    pub http_type: HttpType,
}

impl ProxyPassEndpointInfo {
    pub fn new(host_endpoint: String, http_type: HttpType, debug: bool) -> Self {
        Self {
            host_endpoint,
            debug,
            http_type,
        }
    }

    pub fn is_my_endpoint(&self, other_host_endpoint: &str) -> bool {
        self.host_endpoint == other_host_endpoint
    }

    /*
       pub fn get_port(&self) -> Result<u16, String> {
           let mut elements = self.host_endpoint.split(":");
           let first = elements.next().unwrap();
           if let Some(last) = elements.next() {
               match last.parse::<u16>() {
                   Ok(port) => Ok(port),
                   Err(_) => Err(format!("Can not parse port from {}", self.host_endpoint)),
               }
           } else {
               match first.parse::<u16>() {
                   Ok(port) => Ok(port),
                   Err(_) => Err(format!("Can not parse port from {}", self.host_endpoint)),
               }
           }
       }
    */

    pub fn as_str(&self) -> &str {
        &self.host_endpoint
    }
}
