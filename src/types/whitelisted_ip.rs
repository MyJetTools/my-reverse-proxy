use std::net::IpAddr;

pub trait IntoIp {
    fn get_ip_value(&self) -> u32;
}

impl IntoIp for &'_ str {
    fn get_ip_value(&self) -> u32 {
        let mut result = 0;
        for (i, v) in self.split(".").enumerate() {
            match i {
                0 => result |= v.parse::<u32>().unwrap() << 24,
                1 => result |= v.parse::<u32>().unwrap() << 16,
                2 => result |= v.parse::<u32>().unwrap() << 8,
                3 => result |= v.parse::<u32>().unwrap(),
                _ => panic!("Invalid ip format"),
            }
        }

        result
    }
}

impl IntoIp for &[u8; 4] {
    fn get_ip_value(&self) -> u32 {
        let mut result: u32 = 0;

        result |= (self[0] as u32) << 24;
        result |= (self[1] as u32) << 16;
        result |= (self[2] as u32) << 8;
        result |= (self[3] as u32) << 8;

        result
    }
}

impl IntoIp for IpAddr {
    fn get_ip_value(&self) -> u32 {
        match self {
            IpAddr::V4(ip) => (&ip.octets()).get_ip_value(),
            IpAddr::V6(_) => panic!("Ipv6 is not supported"),
        }
    }
}

pub enum WhitelistedIp {
    SingleIp(u32),
    Range { ip_from: u32, ip_to: u32 },
}

impl WhitelistedIp {
    pub fn is_my_ip(&self, other_ip: &impl IntoIp) -> bool {
        let value = other_ip.get_ip_value();
        match self {
            WhitelistedIp::SingleIp(ip) => *ip == value,
            WhitelistedIp::Range { ip_from, ip_to } => *ip_from <= value && value <= *ip_to,
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_parse_ip() {
        let ip1: [u8; 4] = [192, 168, 1, 1];

        let left = (&ip1).get_ip_value();

        let right = "192.168.1.1".get_ip_value();

        assert_eq!(left, right)
    }
}
