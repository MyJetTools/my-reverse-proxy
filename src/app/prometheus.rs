use prometheus::{Encoder, IntGaugeVec, Opts, Registry, TextEncoder};

pub struct Prometheus {
    pub http1_client_tcp_connects: IntGaugeVec,
    pub http1_client_tcp_read_threads: IntGaugeVec,
    pub http1_client_tcp_write_threads: IntGaugeVec,
    pub http1_client_instances: IntGaugeVec,
    pub http1_client_web_sockets: IntGaugeVec,

    registry: Registry,
}

impl Prometheus {
    pub fn new() -> Self {
        let registry = Registry::new();

        let http1_client_tcp_connects = create_gauge_vec(
            &registry,
            "http1_client_remote_tcp_connects",
            "Http1 TCP connects",
        );

        let http1_client_tcp_read_threads = create_gauge_vec(
            &registry,
            "http1_client_read_threads",
            "Http1 Client Read threads",
        );
        let http1_client_tcp_write_threads = create_gauge_vec(
            &registry,
            "http1_client_write_threads",
            "Http1 Client Write threads",
        );

        let http1_client_instances = create_gauge_vec(
            &registry,
            "http1_client_instances",
            "Http1 Client Instances",
        );

        let http1_client_web_sockets = create_gauge_vec(
            &registry,
            "http1_client_web_sockets",
            "Http1 Client Web Sockets",
        );

        let result = Self {
            http1_client_tcp_connects,
            http1_client_tcp_read_threads,
            http1_client_tcp_write_threads,
            http1_client_web_sockets,
            http1_client_instances,
            registry,
        };

        result
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        buffer
    }
}

fn create_gauge_vec(registry: &Registry, name: &str, description: &str) -> IntGaugeVec {
    let gauge_opts = Opts::new(name, description);
    let labels = &["remote_host"];
    let result = IntGaugeVec::new(gauge_opts, labels).unwrap();

    registry.register(Box::new(result.clone())).unwrap();

    result
}

impl my_http_client::http1::MyHttpClientMetrics for Prometheus {
    fn tcp_connect(&self, name: &str) {
        self.http1_client_tcp_connects
            .with_label_values(&[name])
            .inc();
    }

    fn tcp_disconnect(&self, name: &str) {
        self.http1_client_tcp_connects
            .with_label_values(&[name])
            .dec();
    }

    fn read_thread_start(&self, name: &str) {
        self.http1_client_tcp_read_threads
            .with_label_values(&[name])
            .inc();
    }

    fn read_thread_stop(&self, name: &str) {
        self.http1_client_tcp_read_threads
            .with_label_values(&[name])
            .dec();
    }

    fn write_thread_start(&self, name: &str) {
        self.http1_client_tcp_write_threads
            .with_label_values(&[name])
            .inc();
    }

    fn write_thread_stop(&self, name: &str) {
        self.http1_client_tcp_write_threads
            .with_label_values(&[name])
            .dec();
    }

    fn instance_created(&self, name: &str) {
        self.http1_client_instances.with_label_values(&[name]).inc();
    }

    fn instance_disposed(&self, name: &str) {
        self.http1_client_instances.with_label_values(&[name]).dec();
    }

    fn upgraded_to_websocket(&self, name: &str) {
        self.http1_client_web_sockets
            .with_label_values(&[name])
            .inc();
    }

    fn websocket_is_disconnected(&self, name: &str) {
        self.http1_client_web_sockets
            .with_label_values(&[name])
            .dec();
    }
}
