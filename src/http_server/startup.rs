use std::{net::SocketAddr, sync::Arc};

use my_http_server::{controllers::swagger::SwaggerMiddleware, MyHttpServer};

const DEFAULT_PORT: u16 = 8000;

pub fn start() {
    let http_port = if let Some(listen_port) = crate::app::APP_CTX.http_control_port {
        listen_port
    } else {
        DEFAULT_PORT
    };

    let mut http_server = MyHttpServer::new(SocketAddr::from(([0, 0, 0, 0], http_port)));

    let controllers = Arc::new(super::builder::build_controllers());

    let swagger_middleware = SwaggerMiddleware::new(
        controllers.clone(),
        crate::app::APP_NAME,
        crate::app::APP_VERSION,
    );

    http_server.add_middleware(Arc::new(swagger_middleware));

    http_server.add_middleware(controllers);

    http_server.start(
        crate::app::APP_CTX.states.clone(),
        my_logger::LOGGER.clone(),
    );
}
