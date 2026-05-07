use std::collections::HashMap;

use my_tls::crl::CrlRecord;

use crate::tcp_listener::https::ClientCertificateData;

pub struct ListOfCrl {
    data: HashMap<String, Vec<CrlRecord>>,
}

impl ListOfCrl {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
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
