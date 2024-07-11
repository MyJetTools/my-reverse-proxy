use crate::{types::Email, variables_reader::VariablesReader};
use serde::*;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoogleAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub whitelisted_domains: String,
}

impl GoogleAuthSettings {
    pub fn clone_an_populate(&self, vars: VariablesReader) -> Self {
        let client_id =
            crate::populate_variable::populate_variable(self.client_id.trim(), vars).to_string();
        let client_secret =
            crate::populate_variable::populate_variable(self.client_secret.trim(), vars)
                .to_string();

        let whitelisted_domains =
            crate::populate_variable::populate_variable(self.whitelisted_domains.trim(), vars)
                .to_string();

        Self {
            client_id,
            client_secret,
            whitelisted_domains,
        }
    }
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
