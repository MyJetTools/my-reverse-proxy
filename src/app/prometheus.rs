use prometheus::{Encoder, IntGaugeVec, Opts, Registry, TextEncoder};

pub struct Prometheus {
    pub tcp_connects: IntGaugeVec,
    pub tcp_read_threads: IntGaugeVec,
    pub tcp_write_threads: IntGaugeVec,

    registry: Registry,
}

impl Prometheus {
    pub fn new() -> Self {
        let registry = Registry::new();

        let tcp_connects =
            create_gauge_vec(&registry, "http1_remote_tcp_connects", "Http1 TCP connects");

        let tcp_read_threads =
            create_gauge_vec(&registry, "http1_read_threads", "Http1 Read threads");
        let tcp_write_threads =
            create_gauge_vec(&registry, "http1_write_threads", "Http1 Write threads");

        let result = Self {
            tcp_connects,
            tcp_read_threads,
            tcp_write_threads,
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
        self.tcp_connects.with_label_values(&[name]).inc();
    }

    fn tcp_disconnect(&self, name: &str) {
        self.tcp_connects.with_label_values(&[name]).dec();
    }

    fn read_thread_start(&self, name: &str) {
        self.tcp_read_threads.with_label_values(&[name]).inc();
    }

    fn read_thread_stop(&self, name: &str) {
        self.tcp_read_threads.with_label_values(&[name]).dec();
    }

    fn write_thread_start(&self, name: &str) {
        self.tcp_write_threads.with_label_values(&[name]).inc();
    }

    fn write_thread_stop(&self, name: &str) {
        self.tcp_write_threads.with_label_values(&[name]).dec();
    }
}
