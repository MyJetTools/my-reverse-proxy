use std::sync::Arc;

use rust_extensions::MyTimerTick;

use crate::app::AppContext;

pub struct CrlRefresherTimer {
    app: Arc<AppContext>,
}

impl CrlRefresherTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for CrlRefresherTimer {
    async fn tick(&self) {
        let list_of_crl = self
            .app
            .ssl_certificates_cache
            .read(|config| config.client_ca.get_list_of_crl())
            .await;

        if list_of_crl.len() == 0 {
            return;
        }

        for (id, crl_file_source) in list_of_crl {
            crate::scripts::update_crl(&self.app, id, &crl_file_source).await;
        }
    }
}
