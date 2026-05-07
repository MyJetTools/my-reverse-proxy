pub trait MyHttpClientMetrics {
    fn instance_created(&self, name: &str);
    fn instance_disposed(&self, name: &str);
    fn tcp_connect(&self, name: &str);
    fn tcp_disconnect(&self, name: &str);
    fn read_thread_start(&self, name: &str);
    fn read_thread_stop(&self, name: &str);
    fn write_thread_start(&self, name: &str);
    fn write_thread_stop(&self, name: &str);
    fn upgraded_to_websocket(&self, name: &str);
    fn websocket_is_disconnected(&self, name: &str);
}
