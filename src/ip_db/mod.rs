mod ipv4_data;

use std::net::Ipv4Addr;

use ipv4_data::IPV4_RANGES;

pub fn lookup_country(ip: Ipv4Addr) -> Option<&'static [u8; 2]> {
    let key = u32::from(ip);
    let idx = IPV4_RANGES.partition_point(|(start, _, _)| *start <= key);
    if idx == 0 {
        return None;
    }
    let (start, end, code) = &IPV4_RANGES[idx - 1];
    if key >= *start && key <= *end {
        Some(code)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_below_range_returns_none() {
        assert_eq!(lookup_country(Ipv4Addr::new(0, 0, 0, 0)), None);
    }

    #[test]
    fn lookup_known_ip_returns_two_letter_code() {
        let result = lookup_country(Ipv4Addr::new(8, 8, 8, 8));
        let code = result.expect("8.8.8.8 should be in the database");
        assert_eq!(code.len(), 2);
        assert!(code.iter().all(|b| b.is_ascii_uppercase()));
        let as_str = std::str::from_utf8(code).unwrap();
        assert_eq!(as_str, "US", "8.8.8.8 is Google DNS, should resolve to US");
    }

    #[test]
    fn lookup_first_au_range() {
        let code = lookup_country(Ipv4Addr::new(1, 0, 0, 0))
            .expect("1.0.0.0 should be in the database");
        assert_eq!(std::str::from_utf8(code).unwrap(), "AU");
    }
}
