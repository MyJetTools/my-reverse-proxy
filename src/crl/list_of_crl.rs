use std::collections::HashMap;

use my_tls::crl::CrlRecord;

use crate::{configurations::FileSource, http_server::ClientCertificateData};

pub struct ListOfCrl {
    data: HashMap<String, Vec<CrlRecord>>,
}

impl ListOfCrl {
    pub async fn new(
        src: &HashMap<String, FileSource>,
        init_on_start: bool,
    ) -> Result<Self, String> {
        let mut data = HashMap::new();

        for (name, file_source) in src {
            let content = file_source.load_file_content(None, init_on_start).await?;
            let crl = my_tls::crl::read(&content)?;
            data.insert(name.clone(), crl);
        }

        Ok(Self { data })
    }

    pub fn has_certificate_as_revoked(&self, client_cert: &ClientCertificateData) -> bool {
        let crl = self.data.get(client_cert.ca_id.as_str());

        if crl.is_none() {
            return false;
        }

        let crl = crl.unwrap();

        for record in crl {
            if record.serial == client_cert.serial {
                return true;
            }
        }

        false
    }
}
