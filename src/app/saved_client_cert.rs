use std::{collections::HashMap, sync::Mutex};

pub struct SavedClientCert {
    items: Mutex<HashMap<u16, Vec<(u64, String)>>>,
}

impl SavedClientCert {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
        }
    }

    pub fn save(&self, port: u16, id: u64, cert: String) {
        let mut read_access = self.items.lock().unwrap();

        if !read_access.contains_key(&port) {
            read_access.insert(port, Vec::new());
        }

        let by_port = read_access.get_mut(&port).unwrap();

        let index = by_port.iter().position(|x| x.0 == id);
        if let Some(index) = index {
            by_port.remove(index);
        }
        by_port.push((id, cert));
    }

    pub fn get(&self, port: u16, id: u64) -> Option<String> {
        let mut read_access = self.items.lock().unwrap();

        if let Some(by_port) = read_access.get_mut(&port) {
            let index = by_port.iter().position(|x| x.0 == id)?;
            return Some(by_port.remove(index).1);
        }

        None
    }
}
