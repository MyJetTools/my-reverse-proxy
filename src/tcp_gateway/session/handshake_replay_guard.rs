use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

const DEFAULT_WINDOW: Duration = Duration::from_secs(5);

/// Process-wide guard that protects the gateway handshake against replay
/// attacks.
///
/// Each accepted handshake's timestamp must be:
/// 1. Within `window` of "now" (drift in either direction).
/// 2. Not equal to any timestamp seen within the last `window` seconds.
///
/// This prevents an attacker who has captured an encrypted handshake frame from
/// reusing it later — the timestamp will be too old, and even if replayed
/// immediately, the second copy is rejected as a duplicate.
pub struct HandshakeReplayGuard {
    window_micros: i64,
    inner: Mutex<HandshakeReplayInner>,
}

struct HandshakeReplayInner {
    queue: VecDeque<i64>,
    seen: HashSet<i64>,
}

impl HandshakeReplayGuard {
    pub fn new(window: Duration) -> Self {
        Self {
            window_micros: window.as_micros() as i64,
            inner: Mutex::new(HandshakeReplayInner {
                queue: VecDeque::new(),
                seen: HashSet::new(),
            }),
        }
    }

    pub fn default_window() -> Self {
        Self::new(DEFAULT_WINDOW)
    }

    /// Validate a handshake timestamp. Returns `Ok(())` when accepted; the
    /// timestamp is recorded so any future duplicate will be rejected for the
    /// next `window` seconds.
    pub fn validate(&self, timestamp_micros: i64) -> Result<(), String> {
        let now = DateTimeAsMicroseconds::now().unix_microseconds;
        let mut inner = self.inner.lock();

        // Drop entries older than the window.
        let cutoff = now - self.window_micros;
        while let Some(&front) = inner.queue.front() {
            if front < cutoff {
                inner.queue.pop_front();
                inner.seen.remove(&front);
            } else {
                break;
            }
        }

        // Drift check (replay of old packet, or skewed peer clock).
        let drift = (now - timestamp_micros).abs();
        if drift > self.window_micros {
            return Err(format!(
                "Handshake timestamp drift {}us exceeds window {}us",
                drift, self.window_micros
            ));
        }

        // Duplicate check.
        if !inner.seen.insert(timestamp_micros) {
            return Err(format!(
                "Handshake timestamp {} already seen — replay rejected",
                timestamp_micros
            ));
        }
        inner.queue.push_back(timestamp_micros);

        Ok(())
    }
}

impl Default for HandshakeReplayGuard {
    fn default() -> Self {
        Self::default_window()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_extensions::date_time::DateTimeAsMicroseconds;

    #[test]
    fn accepts_fresh_timestamp() {
        let guard = HandshakeReplayGuard::default_window();
        let ts = DateTimeAsMicroseconds::now().unix_microseconds;
        assert!(guard.validate(ts).is_ok());
    }

    #[test]
    fn rejects_replay_within_window() {
        let guard = HandshakeReplayGuard::default_window();
        let ts = DateTimeAsMicroseconds::now().unix_microseconds;
        assert!(guard.validate(ts).is_ok());
        assert!(guard.validate(ts).is_err(), "second use must be rejected");
    }

    #[test]
    fn rejects_too_old_timestamp() {
        let guard = HandshakeReplayGuard::default_window();
        let now = DateTimeAsMicroseconds::now().unix_microseconds;
        let too_old = now - Duration::from_secs(10).as_micros() as i64;
        assert!(guard.validate(too_old).is_err());
    }

    #[test]
    fn rejects_too_far_future_timestamp() {
        let guard = HandshakeReplayGuard::default_window();
        let now = DateTimeAsMicroseconds::now().unix_microseconds;
        let too_new = now + Duration::from_secs(10).as_micros() as i64;
        assert!(guard.validate(too_new).is_err());
    }
}
