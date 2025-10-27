use std::sync::Arc;

#[derive(Clone)]
pub struct EndpointHttpHostString {
    src: Arc<String>,
    port: u16,
    index: Option<usize>,
}

impl EndpointHttpHostString {
    pub fn new(host: String) -> Result<Self, String> {
        let index = host.find(':');

        let port_str = match index {
            Some(index) => &host[index + 1..],
            None => host.as_str(),
        };

        let port: u16 = match port_str.parse() {
            Ok(result) => result,
            Err(_) => {
                return Err(format!("Can not pars endpoint port for host: {}", host));
            }
        };

        let result = Self {
            src: Arc::new(host),
            port,
            index,
        };

        Ok(result)
    }

    pub fn has_server_name(&self) -> bool {
        self.index.is_some()
    }

    pub fn get_server_name(&self) -> Option<&str> {
        let index = self.index?;
        let result = &self.src[..index];
        Some(result)
    }

    pub fn is_my_server_name(&self, server_name: &str) -> bool {
        match self.get_server_name() {
            Some(my_server_name) => my_server_name.eq_ignore_ascii_case(server_name),
            None => true,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.src
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }
}
