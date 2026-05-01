use ahash::AHashMap;
use parking_lot::Mutex;

pub struct RpsAccumulator {
    by_domain: Mutex<AHashMap<String, u64>>,
}

impl RpsAccumulator {
    pub fn new() -> Self {
        Self {
            by_domain: Mutex::new(AHashMap::new()),
        }
    }

    pub fn inc_domain(&self, domain: &str) {
        let mut m = self.by_domain.lock();
        if let Some(v) = m.get_mut(domain) {
            *v += 1;
        } else {
            m.insert(domain.to_string(), 1);
        }
    }

    /// Snapshot every (domain, count) pair and reset all counts to zero in
    /// place. Keys remain so the gauge falls back to 0 on idle domains
    /// instead of going stale.
    pub fn snapshot_and_reset(&self) -> Vec<(String, u64)> {
        let mut m = self.by_domain.lock();
        let snapshot: Vec<_> = m.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for v in m.values_mut() {
            *v = 0;
        }
        snapshot
    }
}
