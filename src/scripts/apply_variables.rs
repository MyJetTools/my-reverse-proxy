use rust_extensions::{placeholders::ContentToken, StrOrString};

use crate::settings::SettingsModel;

pub fn apply_variables<'s>(
    settings_model: &SettingsModel,
    value: &'s str,
) -> Result<StrOrString<'s>, String> {
    if !value.contains("${") {
        return Ok(value.into());
    }

    let mut result = String::new();

    let placeholders_iterator =
        rust_extensions::placeholders::PlaceholdersIterator::new(value, "${", "}");

    for itm in placeholders_iterator {
        match itm {
            ContentToken::Text(text) => {
                result.push_str(text);
            }
            ContentToken::Placeholder(placeholder) => {
                if let Some(vars) = settings_model.variables.as_ref() {
                    if let Some(value) = vars.get(placeholder) {
                        result.push_str(value.as_str());
                        continue;
                    }
                }
                return Err(format!("Variable {} not found", placeholder));
            }
        }
    }

    Ok(result.into())
}
