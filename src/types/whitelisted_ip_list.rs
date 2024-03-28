use super::{IntoIp, WhitelistedIp};

pub struct WhiteListedIpList {
    items: Vec<WhitelistedIp>,
}

impl WhiteListedIpList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn apply(&mut self, src: Option<&String>) {
        if src.is_none() {
            return;
        }

        for itm in src.unwrap().split(";") {
            let mut parts = itm.split("-");

            let left = parts.next().unwrap();

            if let Some(right) = parts.next() {
                let ip_from = left.get_ip_value();
                let ip_to = right.get_ip_value();

                self.items.push(WhitelistedIp::Range { ip_from, ip_to });
            } else {
                self.items
                    .push(WhitelistedIp::SingleIp(left.get_ip_value()));
            }
        }
    }

    pub fn is_whitelisted(&self, ip: &impl IntoIp) -> bool {
        if self.items.is_empty() {
            return true;
        }

        for itm in &self.items {
            if itm.is_my_ip(ip) {
                return true;
            }
        }

        false
    }
}
