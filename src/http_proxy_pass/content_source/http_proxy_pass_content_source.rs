use bytes::Bytes;
use http_body_util::Full;

use crate::{
    http_content_source::local_path::*, http_content_source::static_content::*,
    http_content_source::*, http_proxy_pass::ProxyPassError,
};

use super::*;

pub enum HttpProxyPassContentSource {
    UnixHttp1(UnixHttp1ContentSource),
    Http1(Http1ContentSource),
    Https1(Https1ContentSource),
    Http2(Http2ContentSource),
    UnixHttp2(UnixHttp2ContentSource),
    Https2(Https2ContentSource),
    //Http1OverGateway(Http1OverGatewayContentSource),
    //Http2OverGateway(Http2OverGatewayContentSource),
    Http1OverSsh(Http1OverSshContentSource),
    Http2OverSsh(Http2OverSshContentSource),
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    PathOverGateway(PathOverGatewayContentSource),
    Static(StaticContentSrc),
}

impl HttpProxyPassContentSource {
    pub async fn send_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        match self {
            Self::Http1(model) => model.execute(req).await,
            Self::UnixHttp2(model) => model.execute(req).await,
            Self::UnixHttp1(model) => model.execute(req).await,
            Self::Https1(model) => model.execute(req).await,
            Self::Http2(model) => model.execute(req).await,
            Self::Https2(model) => model.execute(req).await,
            Self::Http1OverSsh(model) => model.execute(req).await,
            Self::Http2OverSsh(model) => model.execute(req).await,
            Self::LocalPath(model) => model.execute(req).await,
            Self::PathOverSsh(model) => model.execute(req).await,
            //Self::Http1OverGateway(model) => model.execute(req).await,
            //Self::Http2OverGateway(model) => model.execute(req).await,
            Self::PathOverGateway(model) => model.execute(req).await,
            Self::Static(model) => model.execute(req).await,
        }
    }
}
