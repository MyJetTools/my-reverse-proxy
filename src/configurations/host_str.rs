use std::sync::Arc;

#[derive(Clone)]
pub struct EndpointHttpHostString {
    src: Arc<String>,
    port: u16,
}

impl EndpointHttpHostString {
    pub fn new(host: String) -> Result<Self, String> {
        let port: u16 = match host.split(':').last().unwrap().parse() {
            Ok(result) => result,
            Err(_) => {
                return Err(format!("Can not pars endpoint port for host: {}", host));
            }
        };

        let result = Self {
            src: Arc::new(host),
            port,
        };

        Ok(result)
    }

    pub fn is_my_server_name(&self, server_name: &str) -> bool {
        let index = self.src.find(':');

        match index {
            Some(index) => {
                let name = &self.src[..index];
                rust_extensions::str_utils::compare_strings_case_insensitive(name, server_name)
            }
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
