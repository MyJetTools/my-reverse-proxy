use std::sync::Arc;

use my_ssh::{ssh_settings::OverSshConnectionSettings, SshCredentials};
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

pub const GATEWAY_PREFIX: &str = "gateway:";

pub enum MyReverseProxyRemoteEndpoint {
    Gateway {
        id: Arc<String>,
        remote_host: Arc<RemoteEndpointOwned>,
    },
    OverSsh {
        ssh: Arc<SshCredentials>,
        remote_host: Arc<RemoteEndpointOwned>,
    },
    Direct {
        remote_host: Arc<RemoteEndpointOwned>,
    },
}

impl MyReverseProxyRemoteEndpoint {
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
            MyReverseProxyRemoteEndpoint::OverSsh { ssh, remote_host } => {
                format!("ssh:{}->{}", ssh.to_string().as_str(), remote_host.as_str())
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
            Some(ssh) => Ok(MyReverseProxyRemoteEndpoint::OverSsh {
                ssh,
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
