use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use hyper::Uri;
use serde::*;

use crate::ssh_configuration::SshConfiguration;

pub enum ProxyPassRemoteEndpoint {
    Http(Uri),
    Ssh(SshConfiguration),
}

#[derive(my_settings_reader::SettingsModel, Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, Vec<Location>>,
}

impl SettingsReader {
    pub async fn get_configurations(&self, host: &str) -> Vec<(String, ProxyPassRemoteEndpoint)> {
        let mut result = Vec::new();
        let read_access = self.settings.read().await;

        for (settings_host, locations) in &read_access.hosts {
            if rust_extensions::str_utils::compare_strings_case_insensitive(settings_host, host) {
                for location in locations {
                    if location.proxy_pass_to.trim().starts_with("ssh") {
                        result.push((
                            location.location.to_string(),
                            ProxyPassRemoteEndpoint::Ssh(SshConfiguration::parse(
                                &location.proxy_pass_to,
                            )),
                        ));
                    } else {
                        result.push((
                            location.location.to_string(),
                            ProxyPassRemoteEndpoint::Http(
                                Uri::from_str(&location.proxy_pass_to).unwrap(),
                            ),
                        ));
                    }
                }
                break;
            }
        }

        result
    }

    pub async fn get_listen_ports(&self) -> Vec<u16> {
        let read_access = self.settings.read().await;

        let mut result = BTreeMap::new();

        for host in read_access.hosts.keys() {
            let host_port = host.split(':');

            match host_port.last().unwrap().parse::<u16>() {
                Ok(port) => {
                    result.insert(port, ());
                }
                Err(_) => {
                    panic!("Can not read port from host: '{}'", host);
                }
            }
        }

        result.keys().cloned().collect()
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
