use std::collections::HashMap;

use tokio::sync::Mutex;

pub struct FilesCache {
    pub data: Mutex<HashMap<String, Vec<u8>>>,
}

impl FilesCache {
    pub fn new() -> Self {
        FilesCache {
            data: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, file_name: &str) -> Option<Vec<u8>> {
        let read_access = self.data.lock().await;
        let result = read_access.get(file_name)?;
        Some(result.clone())
    }

    pub async fn add(&self, file_name: String, content: Vec<u8>) {
        let mut write_access = self.data.lock().await;
        write_access.insert(file_name, content);
    }
}
