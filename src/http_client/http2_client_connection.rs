use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http2::SendRequest;
use rust_extensions::date_time::DateTimeAsMicroseconds;

pub struct Http2ClientConnection {
    pub connected: DateTimeAsMicroseconds,
    pub send_request: SendRequest<Full<Bytes>>,
}

impl Http2ClientConnection {
    pub fn new(send_request: SendRequest<Full<Bytes>>) -> Self {
        Self {
            connected: DateTimeAsMicroseconds::now(),
            send_request,
        }
    }
}
