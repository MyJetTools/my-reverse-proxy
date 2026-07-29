mod ipv4_data;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ip2location::LocationDB;
use ipv4_data::IPV4_RANGES;

const IPV6_DB_PATH: &str = "ip_v6/IP2LOCATION-LITE-DB1.IPV6.BIN";

static IPV6_DB: std::sync::LazyLock<Option<LocationDB>> =
    std::sync::LazyLock::new(|| match LocationDB::from_file(IPV6_DB_PATH) {
        Ok(db) => {
            println!("[ip_db] loaded IPv6 country DB from {}", IPV6_DB_PATH);
            Some(db)
        }
        Err(e) => {
            eprintln!(
                "[ip_db] failed to load IPv6 DB from {}: {:?}",
                IPV6_DB_PATH, e
            );
            None
        }
    });

pub fn lookup_country(ip: IpAddr) -> Option<[u8; 2]> {
    match ip {
        IpAddr::V4(v4) => lookup_country_v4(v4),
        IpAddr::V6(v6) => lookup_country_v6(v6),
    }
}

/// Resolves the ISO-3 country code (e.g. `USA`) for an IP, used as the flag
/// file name in the UI. The country DB returns ISO-2; `rust_common` maps it to
/// ISO-3.
pub fn lookup_country_iso3(ip: IpAddr) -> Option<String> {
    let code2 = lookup_country(ip)?;
    let iso2 = std::str::from_utf8(&code2).ok()?;
    let country = rust_common::country_code::CountryCode::parse(iso2).ok()?;
    Some(country.as_iso3_str().to_string())
}

fn lookup_country_v4(ip: Ipv4Addr) -> Option<[u8; 2]> {
    let key = u32::from(ip);
    let idx = IPV4_RANGES.partition_point(|(start, _, _)| *start <= key);
    if idx == 0 {
        return None;
    }
    let (start, end, code) = &IPV4_RANGES[idx - 1];
    if key >= *start && key <= *end {
        Some(*code)
    } else {
        None
    }
}

fn lookup_country_v6(ip: Ipv6Addr) -> Option<[u8; 2]> {
    let db = IPV6_DB.as_ref()?;
    let record = db.ip_lookup(IpAddr::V6(ip)).ok()?;
    let country = record.country?;
    let bytes = country.short_name.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    if !bytes[0].is_ascii_uppercase() || !bytes[1].is_ascii_uppercase() {
        return None;
    }
    Some([bytes[0], bytes[1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_below_range_returns_none() {
        assert_eq!(lookup_country(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))), None);
    }

    #[test]
    fn lookup_known_ipv4_returns_two_letter_code() {
        let code = lookup_country(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)))
            .expect("8.8.8.8 should be in the database");
        assert_eq!(std::str::from_utf8(&code).unwrap(), "US");
    }

    #[test]
    fn lookup_first_au_range() {
        let code = lookup_country(IpAddr::V4(Ipv4Addr::new(1, 0, 0, 0)))
            .expect("1.0.0.0 should be in the database");
        assert_eq!(std::str::from_utf8(&code).unwrap(), "AU");
    }

    #[test]
    fn lookup_known_ipv6_returns_us() {
        let google_dns_v6: Ipv6Addr = "2001:4860:4860::8888".parse().unwrap();
        let code = lookup_country(IpAddr::V6(google_dns_v6))
            .expect("Google DNS IPv6 should resolve via ip_v6 DB");
        assert_eq!(std::str::from_utf8(&code).unwrap(), "US");
    }
}
