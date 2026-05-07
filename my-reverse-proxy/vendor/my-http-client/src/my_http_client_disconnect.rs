pub trait MyHttpClientDisconnect {
    fn disconnect(&self);
    fn web_socket_disconnect(&self);
    fn get_connection_id(&self) -> u64;
}
