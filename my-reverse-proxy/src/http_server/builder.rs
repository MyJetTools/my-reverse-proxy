use std::sync::Arc;

use my_http_server::controllers::ControllersMiddleware;

pub fn build_controllers() -> ControllersMiddleware {
    let mut result = ControllersMiddleware::new(None, None);

    //result.register_get_action(Arc::new(
    //    super::controllers::configuration::TestConfigurationAction::new(app.clone()),
    //));

    //result.register_get_action(Arc::new(
    //    super::controllers::configuration::TestAndApplyAction::new(app.clone()),
    //));

    result.register_get_action(Arc::new(
        super::controllers::configuration::GetCurrentConfigAction,
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::ReloadEndpointAction,
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::ReloadPortAction,
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::ReloadUnixHostAction,
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshSslCertificateAction,
    ));

    result.register_post_action(Arc::new(super::controllers::configuration::RefreshCaAction));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshUsersListAction,
    ));

    result.register_post_action(Arc::new(
        super::controllers::configuration::RefreshIpListAction,
    ));

    result.register_get_action(Arc::new(super::controllers::logs::GetPortLogsAction));
    result.register_get_action(Arc::new(super::controllers::logs::GetEndpointLogsAction));
    result.register_get_action(Arc::new(super::controllers::logs::GetLocationLogsAction));
    result.register_post_action(Arc::new(super::controllers::logs::SetEndpointDebugAction));
    result.register_post_action(Arc::new(super::controllers::logs::SetLocationDebugAction));

    result.register_get_action(Arc::new(super::controllers::prometheus::GetMetricsAction));

    result.register_get_action(Arc::new(
        super::controllers::ip_blocklist::CheckIpBlocklistAction,
    ));

    result.register_post_action(Arc::new(super::controllers::ip_blocklist::UnblockIpAction));

    result.register_get_action(Arc::new(
        super::controllers::ssl_certificates::GetCurrentSslCertificatesAction,
    ));

    result.register_post_action(Arc::new(super::controllers::ssh::InitPassKeyAction));

    result.register_get_action(Arc::new(
        super::controllers::debug::GetUpstreamsSnapshotAction,
    ));

    result
}
