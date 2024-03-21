use std::collections::HashMap;

use rust_extensions::StrOrString;

pub const PLACEHOLDER_OPEN_TOKEN: &str = "${";
pub const PLACEHOLDER_CLOSE_TOKEN: &str = "}";

pub fn populate_variable<'s>(
    src: &'s str,
    variables: &Option<HashMap<String, String>>,
) -> StrOrString<'s> {
    let index = src.find(PLACEHOLDER_OPEN_TOKEN);

    if index.is_none() {
        return src.into();
    }
    let mut result = String::new();

    for token in rust_extensions::placeholders::PlaceholdersIterator::new(
        src,
        PLACEHOLDER_OPEN_TOKEN,
        PLACEHOLDER_CLOSE_TOKEN,
    ) {
        match token {
            rust_extensions::placeholders::ContentToken::Text(text) => result.push_str(text),
            rust_extensions::placeholders::ContentToken::Placeholder(placeholder) => {
                if let Some(variables) = variables {
                    if let Some(value) = variables.get(placeholder) {
                        result.push_str(value);
                    }
                }
            }
        }
    }

    result.into()
}
