use serde::*;

use crate::types::Email;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoogleAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub whitelisted_domains: String,
}

impl GoogleAuthSettings {
    pub fn domain_is_allowed(&self, email: &Email) -> bool {
        if self.whitelisted_domains.trim() == "*" {
            return true;
        }

        let email_domain = email.get_domain();

        if email_domain.is_none() {
            return false;
        }

        let email_domain = email_domain.unwrap();

        let separator = if self.whitelisted_domains.contains(',') {
            ','
        } else {
            ';'
        };

        for whitelisted_domain in self.whitelisted_domains.split(separator) {
            if rust_extensions::str_utils::compare_strings_case_insensitive(
                whitelisted_domain,
                email_domain,
            ) {
                return true;
            }
        }

        false
    }
}
