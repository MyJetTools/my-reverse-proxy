use std::sync::Arc;

use rust_extensions::MyTimerTick;

use crate::{app::AppContext, crl::ListOfCrl};

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
        let app_config = self.app.try_get_current_app_configuration().await;

        if app_config.is_none() {
            return;
        }

        let app_config = app_config.unwrap();

        let list_of_crl = ListOfCrl::new(&app_config.crl, false).await.unwrap();

        let mut list_of_crl_access = app_config.list_of_crl.lock().await;
        *list_of_crl_access = list_of_crl;
    }
}
