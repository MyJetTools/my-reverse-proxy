pub enum TcpOrUnixSocket<TTcp, TUnix> {
    Tcp(TTcp),
    #[cfg(unix)]
    Unix(TUnix),
}
