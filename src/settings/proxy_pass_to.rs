use super::SshConfiguration;

pub struct ProxyPassTo(String);

impl ProxyPassTo {
    pub fn new(location: String) -> Self {
        Self(location)
    }

    pub fn is_ssh(&self) -> bool {
        self.0.starts_with("ssh")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_ssh_configuration(&self) -> SshConfiguration {
        SshConfiguration::parse(self.as_str())
    }
}
