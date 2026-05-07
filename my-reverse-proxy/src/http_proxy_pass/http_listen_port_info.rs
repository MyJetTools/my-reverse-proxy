use crate::{configurations::ListenHttpEndpointType, types::ListenHost};

#[derive(Clone)]
pub struct HttpListenPortInfo {
    pub endpoint_type: ListenHttpEndpointType,
    pub listen_host: ListenHost,
}
