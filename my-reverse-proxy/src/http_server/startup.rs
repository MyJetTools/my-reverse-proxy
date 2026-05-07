use std::{net::SocketAddr, sync::Arc};

use my_http_server::{controllers::swagger::SwaggerMiddleware, MyHttpServer, StaticFilesMiddleware};

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

    // Serves the Dioxus SPA out of `./wwwroot`. UI is a separate crate
    // (`my-reverse-proxy-ui`); built artifacts are copied here. Falls back
    // to `index.html` for unknown paths so client-side routing works.
    let static_files = StaticFilesMiddleware::new()
        .add_index_file("index.html")
        .set_not_found_file("index.html".to_string())
        .with_etag();

    http_server.add_middleware(Arc::new(static_files));

    http_server.start(
        crate::app::APP_CTX.states.clone(),
        my_logger::LOGGER.clone(),
    );
}
