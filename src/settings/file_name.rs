use rust_extensions::StrOrString;

pub struct FileName<'s>(&'s str);

impl<'s> FileName<'s> {
    pub fn new(location: &'s str) -> Self {
        Self(location)
    }

    pub fn get_value(&'s self) -> StrOrString<'s> {
        if !self.0.starts_with("~") {
            return self.0.into();
        }

        let home_value = std::env::var("HOME").unwrap();

        self.0.replace("~", home_value.as_str()).into()
    }
}
