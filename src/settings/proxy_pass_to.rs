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
}
