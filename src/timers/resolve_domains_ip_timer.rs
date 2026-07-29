use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::Arc,
};

use rust_extensions::MyTimerTick;

use crate::{app::APP_CTX, configurations::ListenConfiguration};

pub struct ResolveDomainsIpTimer;

#[async_trait::async_trait]
impl MyTimerTick for ResolveDomainsIpTimer {
    async fn tick(&self) {
        resolve_endpoint_domains().await;
    }
}

/// Collects every endpoint host that carries a real domain, resolves each via
/// DNS and stores the result in `APP_CTX.resolved_domain_ips`. Domains that fail
/// to resolve keep their previously known IPs so a transient DNS hiccup does not
/// blank the UI.
pub async fn resolve_endpoint_domains() {
    let domains = collect_endpoint_domains().await;

    let previous = APP_CTX.resolved_domain_ips.snapshot();

    let mut result = HashMap::new();

    for domain in domains {
        match resolve_domain(&domain).await {
            Some(ips) => {
                result.insert(domain, Arc::new(ips));
            }
            None => {
                // keep the last known value on a failed resolution
                if let Some(prev) = previous.get(&domain) {
                    result.insert(domain, prev.clone());
                }
            }
        }
    }

    APP_CTX.resolved_domain_ips.replace(result);
}

async fn collect_endpoint_domains() -> HashSet<String> {
    APP_CTX
        .current_configuration
        .get(|config| {
            let mut domains = HashSet::new();

            for listen in config.listen_tcp_endpoints.values() {
                collect_from_listen(listen, &mut domains);
            }

            for listen in config.listen_unix_socket_endpoints.values() {
                collect_from_listen(listen, &mut domains);
            }

            domains
        })
        .await
}

fn collect_from_listen(listen: &ListenConfiguration, domains: &mut HashSet<String>) {
    match listen {
        ListenConfiguration::Http(config) | ListenConfiguration::Mcp(config) => {
            for endpoint in &config.endpoints {
                push_resolvable_domain(domains, endpoint.host_endpoint.get_server_name());
            }
        }
        ListenConfiguration::Tcp(config) => {
            push_resolvable_domain(domains, config.host_endpoint.get_server_name());
        }
    }
}

fn push_resolvable_domain(domains: &mut HashSet<String>, server_name: Option<&str>) {
    if let Some(domain) = resolvable_domain(server_name) {
        domains.insert(domain.to_string());
    }
}

/// Keeps only hosts that are real, resolvable domains: drops wildcards (`*`),
/// empty server names and bare IP literals (already an IP — nothing to resolve).
fn resolvable_domain(server_name: Option<&str>) -> Option<&str> {
    let name = server_name?;
    if name.is_empty() || name == "*" {
        return None;
    }

    if name.parse::<IpAddr>().is_ok() {
        return None;
    }

    Some(name)
}

async fn resolve_domain(domain: &str) -> Option<Vec<IpAddr>> {
    // Port is irrelevant for A/AAAA resolution, but `lookup_host` needs one.
    let addresses = tokio::net::lookup_host((domain, 0)).await.ok()?;

    let mut ips: Vec<IpAddr> = Vec::new();
    for addr in addresses {
        let ip = addr.ip();
        if !ips.contains(&ip) {
            ips.push(ip);
        }
    }

    if ips.is_empty() {
        None
    } else {
        Some(ips)
    }
}
