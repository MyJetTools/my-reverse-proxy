use tokio::sync::RwLock;

use super::{ClientCertificatesCache, SslCertificatesCache};

pub struct CertificatesCacheInner {
    pub ssl_certs: SslCertificatesCache,
    pub client_ca: ClientCertificatesCache,
}

pub struct CertificatesCache {
    inner: RwLock<CertificatesCacheInner>,
}

impl CertificatesCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(CertificatesCacheInner {
                ssl_certs: SslCertificatesCache::new(),
                client_ca: ClientCertificatesCache::new(),
            }),
        }
    }

    pub async fn read<TResult>(
        &self,
        convert: impl Fn(&CertificatesCacheInner) -> TResult,
    ) -> TResult {
        let inner = self.inner.read().await;
        convert(&inner)
    }

    pub async fn write(&self, convert: impl FnOnce(&mut CertificatesCacheInner)) {
        let mut inner = self.inner.write().await;
        convert(&mut inner)
    }
}
