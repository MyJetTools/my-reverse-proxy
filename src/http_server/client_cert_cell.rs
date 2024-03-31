use std::sync::Mutex;

pub struct ClientCertCell {
    pub value: Mutex<Option<String>>,
}

impl ClientCertCell {
    pub fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }

    pub fn set(&self, value: String) {
        let mut write_access = self.value.lock().unwrap();
        *write_access = Some(value);
    }

    pub fn get(&self) -> Option<String> {
        let mut read_access = self.value.lock().unwrap();
        return read_access.take();
    }
}
