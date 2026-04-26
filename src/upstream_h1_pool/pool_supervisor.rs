use std::sync::Arc;

use my_http_client::http1::MyHttpClientMetrics;

pub type ConnectorFactory<TConnector> = Arc<
    dyn Fn() -> (
            TConnector,
            Arc<dyn MyHttpClientMetrics + Send + Sync + 'static>,
        ) + Send
        + Sync
        + 'static,
>;
