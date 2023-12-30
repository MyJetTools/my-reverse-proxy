use std::{collections::HashMap, str::FromStr};

use hyper::Uri;
use serde::*;
#[derive(my_settings_reader::SettingsModel, Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, Vec<Location>>,
}

impl SettingsReader {
    pub async fn get_configurations(&self, host: &str) -> HashMap<String, Uri> {
        let mut result = HashMap::new();
        let read_access = self.settings.read().await;

        for (settings_host, locations) in &read_access.hosts {
            if rust_extensions::str_utils::compare_strings_case_insensitive(settings_host, host) {
                for location in locations {
                    result.insert(
                        location.location.to_string(),
                        Uri::from_str(&location.proxy_pass_to).unwrap(),
                    );
                }
                break;
            }
        }

        result
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    pub location: String,
    pub proxy_pass_to: String,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::SettingsModel;

    #[test]
    fn test() {
        let mut hosts = HashMap::new();

        hosts.insert(
            "localhost:9000".to_string(),
            vec![super::Location {
                location: "/".to_owned(),
                proxy_pass_to: "https://www.google.com".to_owned(),
            }],
        );

        let model = SettingsModel { hosts };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}
