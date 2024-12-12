use std::sync::Arc;

use my_http_server::controllers::ControllersMiddleware;

use crate::app::AppContext;

pub fn build_controllers(app: &Arc<AppContext>) -> ControllersMiddleware {
    let mut result = ControllersMiddleware::new(None, None);

    result.register_get_action(Arc::new(super::controllers::home::IndexAction::new(
        app.clone(),
    )));

    //result.register_get_action(Arc::new(
    //    super::controllers::configuration::TestConfigurationAction::new(app.clone()),
    //));

    //result.register_get_action(Arc::new(
    //    super::controllers::configuration::TestAndApplyAction::new(app.clone()),
    //));

    result.register_get_action(Arc::new(
        super::controllers::configuration::GetCurrentConfigAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::ReloadEndpointAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::ReloadPortAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshSslCertificateAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshCaAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshUsersListAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshIpListAction::new(app.clone()),
    ));

    result.register_get_action(Arc::new(
        super::controllers::prometheus::GetMetricsAction::new(app.clone()),
    ));

    result.register_get_action(Arc::new(
        super::controllers::ssl_certificates::GetCurrentSslCertificatesAction::new(app.clone()),
    ));

    result.register_post_action(Arc::new(super::controllers::ssh::InitPassKeyAction::new(
        app.clone(),
    )));

    result
}
