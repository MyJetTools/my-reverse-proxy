use rust_extensions::MyTimerTick;

pub struct CrlRefresherTimer;

#[async_trait::async_trait]
impl MyTimerTick for CrlRefresherTimer {
    async fn tick(&self) {
        let list_of_crl = crate::app::APP_CTX
            .ssl_certificates_cache
            .read(|config| config.client_ca.get_list_of_crl())
            .await;

        if list_of_crl.len() == 0 {
            return;
        }

        for (id, crl_file_source) in list_of_crl {
            crate::scripts::update_crl(id, &crl_file_source).await;
        }
    }
}
