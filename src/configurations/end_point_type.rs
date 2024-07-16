use std::sync::Arc;

use super::*;

pub enum EndpointType {
    Http(HttpEndpointInfo),
    Tcp(Arc<TcpEndpointHostConfig>),
    TcpOverSsh(Arc<TcpOverSshEndpointHostConfig>),
}

/*
impl EndpointType {
    pub fn get_my_http_locations(&self, req: &HttpRequestBuilder, is_https: bool) -> bool {
        match self {
            Self::Http(http_endpoint_info) => {
                if req.is_mine(http_endpoint_info.host_endpoint.as_str(), is_https) {
                    return true;
                }
            }

            Self::Tcp(_) => {}
            Self::TcpOverSsh(_) => {}
        }
        false
    }

    fn get_host(&self) -> &HostString {
        match self {
            EndpointType::Http(endpoint_info) => &endpoint_info.host_endpoint,
            EndpointType::Tcp(endpoint_info) => &endpoint_info.host,
            EndpointType::TcpOverSsh(endpoint_info) => &endpoint_info.host,
        }
    }
}
 */
