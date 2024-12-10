use std::collections::{HashMap, HashSet};

use tokio::sync::RwLock;

pub struct AllowedUsersList {
    pub data: RwLock<HashMap<String, HashSet<String>>>,
}

impl AllowedUsersList {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    pub async fn is_allowed(&self, id: &str, user: &str) -> bool {
        let data = self.data.read().await;
        if let Some(users) = data.get(id) {
            return users.contains(user);
        }
        false
    }
}
