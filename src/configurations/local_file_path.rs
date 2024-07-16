use rust_extensions::StrOrString;

pub struct LocalFilePath(String);

impl LocalFilePath {
    pub fn new(location: String) -> Self {
        Self(location)
    }

    pub fn get_value<'s>(&'s self) -> StrOrString<'s> {
        if !self.0.starts_with("~") {
            return self.0.as_str().into();
        }

        let home_value = std::env::var("HOME").unwrap();

        self.0.replace("~", home_value.as_str()).into()
    }
}
