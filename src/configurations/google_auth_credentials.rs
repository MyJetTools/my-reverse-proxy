use std::{collections::HashMap, sync::Arc};

use crate::types::Email;

pub struct GoogleAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub whitelisted_domains: String,
}

impl GoogleAuthCredentials {
    pub fn domain_is_allowed(&self, email: &Email) -> bool {
        if self.whitelisted_domains == "*" {
            return true;
        }

        let email_domain = email.get_domain();

        if email_domain.is_none() {
            return false;
        }

        let email_domain = email_domain.unwrap();

        let separator = if self.whitelisted_domains.as_str().contains(',') {
            ','
        } else {
            ';'
        };

        for whitelisted_domain in self.whitelisted_domains.as_str().split(separator) {
            if rust_extensions::str_utils::compare_strings_case_insensitive(
                whitelisted_domain.trim(),
                email_domain,
            ) {
                return true;
            }
        }

        false
    }
}

pub struct GoogleAuthCredentialsList {
    items: HashMap<String, Arc<GoogleAuthCredentials>>,
}

impl GoogleAuthCredentialsList {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn add_or_update(&mut self, key: String, item: GoogleAuthCredentials) {
        self.items.insert(key, Arc::new(item));
    }

    pub fn get(&self, key: &str) -> Option<Arc<GoogleAuthCredentials>> {
        self.items.get(key).cloned()
    }

    pub fn has_credentials(&self, key: &str) -> bool {
        self.items.contains_key(key)
    }
}
