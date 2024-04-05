use std::sync::Arc;

use my_http_server::controllers::ControllersMiddleware;

use crate::app::AppContext;

pub fn build_controllers(app: &Arc<AppContext>) -> ControllersMiddleware {
    let mut result = ControllersMiddleware::new(None, None);

    result.register_get_action(Arc::new(
        super::controllers::configuration::TestConfigurationAction::new(app.clone()),
    ));

    result.register_get_action(Arc::new(
        super::controllers::configuration::TestAndApplyAction::new(app.clone()),
    ));

    result.register_get_action(Arc::new(
        super::controllers::configuration::GetCurrentConfigAction::new(app.clone()),
    ));

    result
}
