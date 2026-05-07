#[derive(Clone, Debug)]
pub struct Email(String);

impl Email {
    pub fn new(data: String) -> Self {
        Self(data)
    }

    pub fn get_domain(&self) -> Option<&str> {
        let index = self.0.find('@')?;

        return Some(&self.0[index + 1..]);
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
