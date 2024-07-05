use std::collections::HashMap;

use rust_extensions::StrOrString;

#[derive(Clone, Copy)]
pub struct VariablesReader<'s> {
    data: &'s Option<HashMap<String, String>>,
}

impl<'s> VariablesReader<'s> {
    pub fn get(&self, key: &str) -> Option<StrOrString> {
        if let Some(data) = self.data {
            let result = data.get(key);

            if let Some(result) = result {
                return Some(result.into());
            }
        }

        match std::env::var(key) {
            Ok(value) => Some(value.into()),
            Err(_) => None,
        }
    }
}

impl<'s> Into<VariablesReader<'s>> for &'s Option<HashMap<String, String>> {
    fn into(self) -> VariablesReader<'s> {
        VariablesReader { data: self }
    }
}
