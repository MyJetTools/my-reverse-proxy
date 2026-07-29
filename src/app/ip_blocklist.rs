use std::net::IpAddr;
use std::sync::Arc;

use ahash::AHashMap;
use arc_swap::ArcSwapOption;
use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::types::WhiteListedIpList;

const WINDOW_SECS: i64 = 15;
/// Unambiguous abuse (e.g. non-TLS bytes on a TLS port) — block quickly.
const HARD_FAIL_THRESHOLD: u16 = 5;
/// Noisier, possibly-benign failures on a real endpoint — must pile up.
/// 30 over the 15s window ≈ 2 requests/sec sustained.
const SOFT_FAIL_THRESHOLD: u16 = 30;
const BLOCK_SECS: i64 = 5 * 60;

/// How strong a signal a failed attempt is. Hard failures are unambiguous abuse
/// we want blocked fast; soft failures are noisy and must accumulate.
#[derive(Clone, Copy, Debug)]
pub enum FailureSeverity {
    Hard,
    Soft,
}

#[derive(Clone, Copy)]
struct IpEntry {
    hard_count: u16,
    soft_count: u16,
    window_start: DateTimeAsMicroseconds,
    blocked_until: Option<DateTimeAsMicroseconds>,
}

pub struct IpBlocklist {
    map: Mutex<AHashMap<IpAddr, IpEntry>>,
    /// Global allow-list: IPs that are never auto-blocked and whose failures are
    /// never counted. `None` when not configured. Behind ArcSwap so it stays
    /// cheap to read on the hot accept path.
    white_list: ArcSwapOption<WhiteListedIpList>,
}

impl IpBlocklist {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(AHashMap::new()),
            white_list: ArcSwapOption::empty(),
        }
    }

    /// Replace the global allow-list. `None` or an empty list disables it — an
    /// empty `WhiteListedIpList` would otherwise match every IP and silently
    /// switch the whole block-list off.
    pub fn set_white_list(&self, src: Option<Vec<String>>) {
        match src {
            Some(src) if !src.is_empty() => {
                self.white_list
                    .store(Some(Arc::new(WhiteListedIpList::new(&src))));
            }
            _ => self.white_list.store(None),
        }
    }

    pub fn is_white_listed(&self, ip: &IpAddr) -> bool {
        match self.white_list.load_full() {
            Some(white_list) => white_list.is_whitelisted(ip),
            None => false,
        }
    }

    pub fn is_blocked(&self, ip: &IpAddr) -> bool {
        if self.is_white_listed(ip) {
            return false;
        }
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

    pub fn unblock(&self, ip: &IpAddr) -> bool {
        let mut map = self.map.lock();
        map.remove(ip).is_some()
    }

    pub fn register_failure(&self, ip: IpAddr, severity: FailureSeverity) {
        if self.is_white_listed(&ip) {
            return;
        }

        let now = DateTimeAsMicroseconds::now();

        let (newly_blocked, hard_count, soft_count) = {
            let mut map = self.map.lock();
            let entry = map.entry(ip).or_insert(IpEntry {
                hard_count: 0,
                soft_count: 0,
                window_start: now,
                blocked_until: None,
            });

            if now.duration_since(entry.window_start).get_full_seconds() > WINDOW_SECS {
                entry.hard_count = 0;
                entry.soft_count = 0;
                entry.window_start = now;
            }

            let was_blocked = match entry.blocked_until {
                Some(until) => until > now,
                None => false,
            };

            match severity {
                FailureSeverity::Hard => entry.hard_count = entry.hard_count.saturating_add(1),
                FailureSeverity::Soft => entry.soft_count = entry.soft_count.saturating_add(1),
            }

            if entry.hard_count >= HARD_FAIL_THRESHOLD || entry.soft_count >= SOFT_FAIL_THRESHOLD {
                let mut blocked_until = now;
                blocked_until.add_seconds(BLOCK_SECS);
                entry.blocked_until = Some(blocked_until);
            }

            let blocked_now = match entry.blocked_until {
                Some(until) => until > now,
                None => false,
            };

            (
                blocked_now && !was_blocked,
                entry.hard_count,
                entry.soft_count,
            )
        };

        // One line per new block (not per failure), with the offending IP.
        if newly_blocked {
            crate::app::APP_CTX.proxy_logs.write_port(
                "ip-blocklist",
                Some(ip.to_string()),
                format!(
                    "Blocked IP {ip} for {BLOCK_SECS}s ({hard_count} hard / {soft_count} soft failures within {WINDOW_SECS}s)"
                ),
            );
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
