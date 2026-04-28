use std::collections::HashMap;

use crate::configurations::ProxyPassToConfig;
use crate::network_stream::*;

use super::*;

pub enum UpstreamState {
    Unknown,
    Http(HashMap<i64, Upstream>),
    Mcp {
        location_id: i64,
        upstream: Upstream,
        mcp_path: String,
    },
}

pub struct UpstreamAccess<'a> {
    pub upstream: &'a mut Upstream,
    pub mcp_path: Option<&'a str>,
}

impl UpstreamState {
    pub fn new() -> Self {
        UpstreamState::Unknown
    }

    pub async fn get_or_connect<
        WritePart: NetworkStreamWritePart + Send + Sync + 'static,
        ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    >(
        &mut self,
        proxy_pass_to: &ProxyPassToConfig,
        location_id: i64,
        ctx: &Http1ServerConnectionContext<WritePart, ReadPart>,
    ) -> Result<UpstreamAccess<'_>, NetworkError> {
        if matches!(proxy_pass_to, ProxyPassToConfig::McpHttp1(_)) {
            // Reuse the existing mcp upstream if it serves the same location.
            let same_location = matches!(
                self,
                UpstreamState::Mcp { location_id: lid, .. } if *lid == location_id
            );

            if !same_location {
                let upstream = Upstream::connect(proxy_pass_to, ctx).await?;
                let mcp_path = match proxy_pass_to {
                    ProxyPassToConfig::McpHttp1(model) => {
                        model.remote_host.get_path_and_query().to_string()
                    }
                    _ => unreachable!(),
                };
                *self = UpstreamState::Mcp {
                    location_id,
                    upstream,
                    mcp_path,
                };
            }

            if let UpstreamState::Mcp {
                upstream, mcp_path, ..
            } = self
            {
                return Ok(UpstreamAccess {
                    upstream,
                    mcp_path: Some(mcp_path.as_str()),
                });
            }
            unreachable!();
        }

        // http branch
        if !matches!(self, UpstreamState::Http(_)) {
            *self = UpstreamState::Http(HashMap::new());
        }

        let map = match self {
            UpstreamState::Http(map) => map,
            _ => unreachable!(),
        };

        if !map.contains_key(&location_id) {
            let upstream = Upstream::connect(proxy_pass_to, ctx).await?;
            map.insert(location_id, upstream);
        }

        Ok(UpstreamAccess {
            upstream: map.get_mut(&location_id).unwrap(),
            mcp_path: None,
        })
    }

    pub fn discard(&mut self, proxy_pass_to: &ProxyPassToConfig, location_id: i64) {
        let request_is_mcp = matches!(proxy_pass_to, ProxyPassToConfig::McpHttp1(_));
        match self {
            UpstreamState::Mcp { .. } if request_is_mcp => {
                *self = UpstreamState::Unknown;
            }
            UpstreamState::Http(map) if !request_is_mcp => {
                map.remove(&location_id);
            }
            _ => {}
        }
    }

    pub fn take_http(&mut self, location_id: i64) -> Option<Upstream> {
        match self {
            UpstreamState::Http(map) => map.remove(&location_id),
            _ => None,
        }
    }
}
