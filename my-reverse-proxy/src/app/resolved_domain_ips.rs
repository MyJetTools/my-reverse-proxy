use std::{collections::HashMap, net::IpAddr, sync::Arc};

use arc_swap::ArcSwap;

/// DNS-resolution cache for endpoint domains. `ResolveDomainsIpTimer` walks every
/// endpoint host that carries a real domain, resolves it and stores the result
/// here; the admin UI reads it to show the IP a domain currently points to next
/// to each endpoint (e.g. `my-host.example.com:443 (12.34.54.34)`).
///
/// The whole snapshot is swapped atomically on each refresh — reads happen on
/// every `/api/configuration/Current` poll, writes once per timer tick — so an
/// `ArcSwap` over an immutable map fits.
pub struct ResolvedDomainIps {
    data: ArcSwap<HashMap<String, Arc<Vec<IpAddr>>>>,
}

impl ResolvedDomainIps {
    pub fn new() -> Self {
        Self {
            data: ArcSwap::from_pointee(HashMap::new()),
        }
    }

    /// Current snapshot — used by the resolver timer to carry over the last
    /// known IPs for domains that fail to resolve on a given tick.
    pub fn snapshot(&self) -> Arc<HashMap<String, Arc<Vec<IpAddr>>>> {
        self.data.load_full()
    }

    pub fn replace(&self, new_map: HashMap<String, Arc<Vec<IpAddr>>>) {
        self.data.store(Arc::new(new_map));
    }

    /// Resolved IPs for a domain formatted for display (e.g. `12.34.54.34` or
    /// `12.34.54.34, 12.34.54.35`). `None` if the domain has not been resolved.
    pub fn get_display(&self, domain: &str) -> Option<String> {
        let map = self.data.load();
        let ips = map.get(domain)?;
        if ips.is_empty() {
            return None;
        }

        let mut result = String::new();
        for ip in ips.iter() {
            if !result.is_empty() {
                result.push_str(", ");
            }
            result.push_str(&ip.to_string());
        }

        Some(result)
    }
}
