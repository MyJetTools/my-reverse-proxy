use std::sync::Arc;

use my_ssh::{ssh_settings::OverSshConnectionSettings, SshCredentials};
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::settings::SettingsModel;

pub const GATEWAY_PREFIX: &str = "gateway:";

pub enum MyReverseProxyRemoteEndpoint {
    Gateway {
        id: Arc<String>,
        remote_host: Arc<RemoteEndpointOwned>,
    },
    OverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: Arc<RemoteEndpointOwned>,
    },
    Direct {
        remote_host: Arc<RemoteEndpointOwned>,
    },
}

impl MyReverseProxyRemoteEndpoint {
    pub async fn try_parse(
        remote_host: &str,
        settings_model: &SettingsModel,
    ) -> Result<Self, String> {
        if remote_host.starts_with(GATEWAY_PREFIX) {
            MyReverseProxyRemoteEndpoint::try_parse_gateway_source(remote_host)
        } else {
            let over_ssh_connection = OverSshConnectionSettings::try_parse(remote_host);

            if over_ssh_connection.is_none() {
                return Err(format!("Invalid remote host {}", remote_host));
            }

            let over_ssh_connection = crate::scripts::ssh::enrich_with_private_key_or_password(
                over_ssh_connection.unwrap(),
                settings_model,
            )
            .await?;

            over_ssh_connection.try_into()
        }
    }

    pub fn try_parse_gateway_source(src: &str) -> Result<Self, String> {
        let mut src_split = src.split("->");

        let left = src_split.next().unwrap();

        let right = src_split.next();

        if right.is_none() {
            return Err(format!("Invalid gateway source: {}", src));
        }

        let right = right.unwrap();

        let gateway_id = get_gateway_id(left)?;

        let remote_host = RemoteEndpointOwned::try_parse(right.to_string())?;

        Ok(Self::Gateway {
            id: gateway_id.to_string().into(),
            remote_host: remote_host.into(),
        })
    }

    pub fn to_string(&self) -> String {
        match self {
            MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                format!("{GATEWAY_PREFIX}{}->{}", id, remote_host.as_str())
            }
            MyReverseProxyRemoteEndpoint::OverSsh {
                ssh_credentials,
                remote_host,
            } => {
                format!(
                    "ssh:{}->{}",
                    ssh_credentials.to_string().as_str(),
                    remote_host.as_str()
                )
            }
            MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                remote_host.as_str().to_string()
            }
        }
    }
}

impl TryInto<MyReverseProxyRemoteEndpoint> for OverSshConnectionSettings {
    type Error = String;

    fn try_into(self) -> Result<MyReverseProxyRemoteEndpoint, Self::Error> {
        match self.ssh_credentials {
            Some(ssh_credentials) => Ok(MyReverseProxyRemoteEndpoint::OverSsh {
                ssh_credentials,
                remote_host: RemoteEndpointOwned::try_parse(self.remote_resource_string)?.into(),
            }),
            None => Ok(MyReverseProxyRemoteEndpoint::Direct {
                remote_host: RemoteEndpointOwned::try_parse(self.remote_resource_string)?.into(),
            }),
        }
    }
}

fn get_gateway_id(src: &str) -> Result<&str, String> {
    let mut splitted = src.split(":");

    let _ = splitted.next().unwrap();

    let next = splitted.next();

    if next.is_none() {
        return Err(format!("Can not extract id from gateway prefix: '{}'", src));
    }

    Ok(next.unwrap())
}
