use ahash::AHashMap;
use parking_lot::Mutex;

#[derive(Default, Clone, Copy)]
pub struct TrafficStats {
    pub c2s_events: u64,
    pub c2s_bytes: u64,
    pub s2c_events: u64,
    pub s2c_bytes: u64,
    pub ws_c2s_events: u64,
    pub ws_c2s_bytes: u64,
    pub ws_s2c_events: u64,
    pub ws_s2c_bytes: u64,
}

pub struct TrafficAccumulator {
    by_domain: Mutex<AHashMap<String, TrafficStats>>,
}

impl TrafficAccumulator {
    pub fn new() -> Self {
        Self {
            by_domain: Mutex::new(AHashMap::new()),
        }
    }

    pub fn record_c2s(&self, domain: &str, bytes: u64) {
        let mut m = self.by_domain.lock();
        if let Some(entry) = m.get_mut(domain) {
            entry.c2s_events = entry.c2s_events.saturating_add(1);
            entry.c2s_bytes = entry.c2s_bytes.saturating_add(bytes);
        } else {
            m.insert(
                domain.to_string(),
                TrafficStats {
                    c2s_events: 1,
                    c2s_bytes: bytes,
                    ..Default::default()
                },
            );
        }
    }

    pub fn record_s2c(&self, domain: &str, bytes: u64) {
        let mut m = self.by_domain.lock();
        if let Some(entry) = m.get_mut(domain) {
            entry.s2c_events = entry.s2c_events.saturating_add(1);
            entry.s2c_bytes = entry.s2c_bytes.saturating_add(bytes);
        } else {
            m.insert(
                domain.to_string(),
                TrafficStats {
                    s2c_events: 1,
                    s2c_bytes: bytes,
                    ..Default::default()
                },
            );
        }
    }

    pub fn record_ws_c2s(&self, domain: &str, bytes: u64) {
        let mut m = self.by_domain.lock();
        if let Some(entry) = m.get_mut(domain) {
            entry.ws_c2s_events = entry.ws_c2s_events.saturating_add(1);
            entry.ws_c2s_bytes = entry.ws_c2s_bytes.saturating_add(bytes);
        } else {
            m.insert(
                domain.to_string(),
                TrafficStats {
                    ws_c2s_events: 1,
                    ws_c2s_bytes: bytes,
                    ..Default::default()
                },
            );
        }
    }

    pub fn record_ws_s2c(&self, domain: &str, bytes: u64) {
        let mut m = self.by_domain.lock();
        if let Some(entry) = m.get_mut(domain) {
            entry.ws_s2c_events = entry.ws_s2c_events.saturating_add(1);
            entry.ws_s2c_bytes = entry.ws_s2c_bytes.saturating_add(bytes);
        } else {
            m.insert(
                domain.to_string(),
                TrafficStats {
                    ws_s2c_events: 1,
                    ws_s2c_bytes: bytes,
                    ..Default::default()
                },
            );
        }
    }

    pub fn snapshot_and_reset(&self) -> Vec<(String, TrafficStats)> {
        let mut m = self.by_domain.lock();
        let snapshot: Vec<_> = m.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for v in m.values_mut() {
            *v = TrafficStats::default();
        }
        snapshot
    }
}
