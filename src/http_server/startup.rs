use std::{net::SocketAddr, sync::Arc};

use my_http_server::{controllers::swagger::SwaggerMiddleware, MyHttpServer};

use crate::app::AppContext;

const DEFAULT_PORT: u16 = 8000;

pub fn start(app: &Arc<AppContext>, listen_port: Option<u16>) {
    let http_port = if let Some(listen_port) = listen_port {
        listen_port
    } else {
        DEFAULT_PORT
    };

    let mut http_server = MyHttpServer::new(SocketAddr::from(([0, 0, 0, 0], http_port)));

    let controllers = Arc::new(super::builder::build_controllers(&app));

    let swagger_middleware = SwaggerMiddleware::new(
        controllers.clone(),
        crate::app::APP_NAME.to_string(),
        crate::app::APP_VERSION.to_string(),
    );

    http_server.add_middleware(Arc::new(swagger_middleware));

    http_server.add_middleware(controllers);

    http_server.start(app.states.clone(), my_logger::LOGGER.clone());
}
