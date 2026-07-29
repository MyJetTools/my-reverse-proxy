use rust_extensions::date_time::DateTimeAsMicroseconds;

/// Wall-clock seconds since the unix epoch.
///
/// OAuth speaks in unix seconds (`exp`, `expires_in`), so the conversion happens
/// once here rather than at every call site.
pub fn now_unix_seconds() -> i64 {
    DateTimeAsMicroseconds::now().unix_microseconds / 1_000_000
}
