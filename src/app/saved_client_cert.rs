use std::{sync::Mutex, thread, time::Duration};

pub struct SavedClientCert {
    items: Mutex<Vec<(i64, String)>>,
}

impl SavedClientCert {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
        }
    }

    pub fn wait_while_we_read_it(&self, id: i64) {
        loop {
            let we_have_it = {
                let we_have_it = false;

                let read_access = self.items.lock().unwrap();

                for itm in read_access.iter() {
                    if itm.0 == id {
                        break;
                    }
                }

                we_have_it
            };

            if !we_have_it {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn save(&self, id: i64, cert: String) {
        let mut items = self.items.lock().unwrap();
        let index = items.iter().position(|x| x.0 == id);
        if let Some(index) = index {
            items.remove(index);
        }
        items.push((id, cert));
    }

    pub fn get(&self, id: i64) -> Option<String> {
        let mut items = self.items.lock().unwrap();
        let index = items.iter().position(|x| x.0 == id)?;
        Some(items.remove(index).1)
    }
}
