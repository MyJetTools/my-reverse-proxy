use std::{collections::HashMap, mem};

use rust_common::placeholders::*;

use crate::settings::SettingsModel;

#[derive(Debug, Default)]
pub struct VariablesCompiled {
    data: HashMap<String, String>,
}

impl VariablesCompiled {
    pub fn merge(&mut self, settings: &mut SettingsModel) {
        if let Some(variables) = mem::take(&mut settings.variables) {
            for (key, value) in variables {
                if let Some(old_value) = self.data.get(&key) {
                    println!("Overriding variable '{old_value}' to '{value}' for key '{key}'");
                }
                self.data.insert(key, value);
            }
        }
    }

    pub fn apply_variables_opt(&self, value: Option<String>) -> Result<Option<String>, String> {
        let value = match value {
            Some(value) => value,
            None => return Ok(None),
        };

        let result = self.apply_variables(value)?;
        Ok(Some(result.to_string()))
    }

    pub fn apply_variables(&self, value: String) -> Result<String, String> {
        if !value.contains("${") {
            return Ok(value.into());
        }

        let mut result = String::new();

        let placeholders_iterator =
            rust_common::placeholders::PlaceholdersIterator::new(&value, "${", "}");

        for itm in placeholders_iterator {
            match itm {
                ContentToken::Text(text) => {
                    result.push_str(text);
                }
                ContentToken::Placeholder(placeholder) => {
                    match name_characteristics(placeholder)? {
                        VariableNameType::Reserved => {
                            result.push_str("${");
                            result.push_str(placeholder);
                            result.push('}');
                            continue;
                        }
                        VariableNameType::Variable => {
                            if let Some(value) = self.data.get(placeholder) {
                                result.push_str(value.as_str());
                                continue;
                            }

                            if let Ok(value) = std::env::var(placeholder) {
                                result.push_str(value.as_str());
                                continue;
                            }
                        }
                    }

                    return Err(format!("Variable {} not found", placeholder));
                }
            }
        }

        Ok(result)
    }
}

pub enum VariableNameType {
    Reserved,
    Variable,
}

fn name_characteristics(src: &str) -> Result<VariableNameType, String> {
    let mut has_upper_case = false;
    let mut has_lower_case = false;

    for c in src.chars() {
        if c.is_ascii_uppercase() {
            has_upper_case = true;
        } else if c.is_ascii_lowercase() {
            has_lower_case = true;
        }

        if has_upper_case && has_lower_case {
            return Err(format!(
                "Env variable {} has to be ether UPPER_CASED or lower_cased",
                src
            ));
        }
    }

    if has_upper_case {
        return Ok(VariableNameType::Reserved);
    }

    Ok(VariableNameType::Variable)
}
