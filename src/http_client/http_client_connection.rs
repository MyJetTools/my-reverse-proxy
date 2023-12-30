use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::SendRequest;
use rust_extensions::date_time::DateTimeAsMicroseconds;

pub struct HttpClientConnection {
    pub connected: DateTimeAsMicroseconds,
    pub send_request: SendRequest<Full<Bytes>>,
}

impl HttpClientConnection {
    pub fn new(send_request: SendRequest<Full<Bytes>>) -> Self {
        Self {
            connected: DateTimeAsMicroseconds::now(),
            send_request,
        }
    }
}
