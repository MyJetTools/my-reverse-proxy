use std::collections::HashMap;

use parking_lot::Mutex;

#[derive(Default)]
struct PendingParts {
    cert_pem: Option<Vec<u8>>,
    private_key_pem: Option<Vec<u8>>,
}

/// Result of staging one part of a two-step manual certificate upload.
pub enum PendingUploadState {
    /// The private key is present but the certificate has not been uploaded yet.
    WaitingForCertificate,
    /// The certificate is present but the private key has not been uploaded yet.
    WaitingForPrivateKey,
    /// Both parts are present and have been removed from the pending store, ready to install.
    Complete {
        cert_pem: Vec<u8>,
        private_key_pem: Vec<u8>,
    },
}

/// Staging area for the two-step manual certificate upload: the certificate PEM and the private
/// key PEM arrive in separate requests (each carrying the raw PEM as its whole body), keyed by
/// cert id. Once both parts are present they are taken out and installed together.
pub struct PendingSslUploads {
    inner: Mutex<HashMap<String, PendingParts>>,
}

impl PendingSslUploads {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_certificate(&self, cert_id: &str, cert_pem: Vec<u8>) -> PendingUploadState {
        self.set(cert_id, |parts| parts.cert_pem = Some(cert_pem))
    }

    pub fn set_private_key(&self, cert_id: &str, private_key_pem: Vec<u8>) -> PendingUploadState {
        self.set(cert_id, |parts| parts.private_key_pem = Some(private_key_pem))
    }

    fn set(&self, cert_id: &str, apply: impl FnOnce(&mut PendingParts)) -> PendingUploadState {
        let mut map = self.inner.lock();

        {
            let parts = map.entry(cert_id.to_string()).or_default();
            apply(parts);
        }

        let parts = map
            .get(cert_id)
            .expect("part was just inserted for this cert_id");

        match (parts.cert_pem.is_some(), parts.private_key_pem.is_some()) {
            (true, true) => {
                let parts = map.remove(cert_id).unwrap();
                PendingUploadState::Complete {
                    cert_pem: parts.cert_pem.unwrap(),
                    private_key_pem: parts.private_key_pem.unwrap(),
                }
            }
            (true, false) => PendingUploadState::WaitingForPrivateKey,
            (false, true) => PendingUploadState::WaitingForCertificate,
            (false, false) => unreachable!("at least one part was just set"),
        }
    }
}
