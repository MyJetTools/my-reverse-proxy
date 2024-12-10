use std::{collections::HashMap, sync::Arc};

use crate::types::{IntoIp, WhiteListedIpList};

pub struct WhiteListedIpListConfigurations {
    data: HashMap<String, Arc<WhiteListedIpList>>,
}

impl WhiteListedIpListConfigurations {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert_or_update(&mut self, id: String, white_list_ip: WhiteListedIpList) {
        self.data.insert(id, Arc::new(white_list_ip));
    }

    pub fn has(&self, id: &str) -> bool {
        self.data.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Option<Arc<WhiteListedIpList>> {
        self.data.get(id).cloned()
    }

    pub fn is_white_listed(&self, id: &str, ip: &impl IntoIp) -> bool {
        if let Some(white_list) = self.data.get(id) {
            return white_list.is_whitelisted(ip);
        }

        false
    }
}
