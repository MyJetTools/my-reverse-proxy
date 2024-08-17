use std::collections::HashMap;

use my_tls::crl::CrlRecord;

use crate::configurations::FileSource;

pub struct ListOfCrl {
    data: HashMap<String, Vec<CrlRecord>>,
}

impl ListOfCrl {
    pub async fn new(src: &HashMap<String, FileSource>) -> Result<Self, String> {
        let mut data = HashMap::new();

        for (name, file_source) in src {
            let content = file_source.load_file_content(None).await?;
            let crl = my_tls::crl::read(&content)?;
            data.insert(name.clone(), crl);
        }

        Ok(Self { data })
    }

    pub async fn has_certificate_as_revoked(&self, name: &str, serial_number: u64) -> bool {
        let crl = self.data.get(name);

        if crl.is_none() {
            return false;
        }

        let crl = crl.unwrap();

        for record in crl {
            if record.serial == serial_number {
                return true;
            }
        }

        false
    }
}
