use std::net::IpAddr;

use ahash::AHashMap;
use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

const WINDOW_SECS: i64 = 60;
const FAIL_THRESHOLD: u16 = 10;
const BLOCK_SECS: i64 = 5 * 60;

#[derive(Clone, Copy)]
struct IpEntry {
    fail_count: u16,
    window_start: DateTimeAsMicroseconds,
    blocked_until: Option<DateTimeAsMicroseconds>,
}

pub struct IpBlocklist {
    map: Mutex<AHashMap<IpAddr, IpEntry>>,
}

impl IpBlocklist {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(AHashMap::new()),
        }
    }

    pub fn is_blocked(&self, ip: &IpAddr) -> bool {
        let map = self.map.lock();
        let Some(entry) = map.get(ip) else {
            return false;
        };
        let Some(blocked_until) = entry.blocked_until else {
            return false;
        };
        blocked_until > DateTimeAsMicroseconds::now()
    }

    pub fn register_success(&self, ip: &IpAddr) {
        let mut map = self.map.lock();
        map.remove(ip);
    }

    pub fn register_failure(&self, ip: IpAddr) {
        let now = DateTimeAsMicroseconds::now();
        let mut map = self.map.lock();
        let entry = map.entry(ip).or_insert(IpEntry {
            fail_count: 0,
            window_start: now,
            blocked_until: None,
        });

        if now.duration_since(entry.window_start).get_full_seconds() > WINDOW_SECS {
            entry.fail_count = 0;
            entry.window_start = now;
        }

        entry.fail_count = entry.fail_count.saturating_add(1);

        if entry.fail_count >= FAIL_THRESHOLD {
            let mut blocked_until = now;
            blocked_until.add_seconds(BLOCK_SECS);
            entry.blocked_until = Some(blocked_until);
        }
    }

    pub fn cleanup(&self) -> usize {
        let now = DateTimeAsMicroseconds::now();
        let mut map = self.map.lock();
        map.retain(|_, entry| {
            if let Some(blocked_until) = entry.blocked_until {
                if blocked_until > now {
                    return true;
                }
            }
            now.duration_since(entry.window_start).get_full_seconds() <= WINDOW_SECS
        });
        map.values()
            .filter(|entry| match entry.blocked_until {
                Some(until) => until > now,
                None => false,
            })
            .count()
    }
}
