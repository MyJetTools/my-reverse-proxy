use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::body::{Body, Frame, SizeHint};

/// A response body that carries an opaque guard whose `Drop` runs only after
/// the body is fully consumed/dropped.
///
/// Used to tie an upstream H1 connection handle (a pool rent, or a dedicated
/// client `Arc`) to the lifetime of the response body. Without this, the proxy
/// dropped the handle as soon as the response *headers* arrived — releasing the
/// pool entry / disposing the client while an (often infinite) SSE body was
/// still streaming. See `attach_conn_guard`.
pub struct GuardedBody {
    inner: BoxBody<Bytes, String>,
    _guard: Box<dyn Send + Sync>,
}

impl Body for GuardedBody {
    type Data = Bytes;
    type Error = String;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // GuardedBody is Unpin (both fields are), so get_mut is safe.
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

/// Wraps `resp`'s body so `guard` is dropped only when the body is. `guard` is
/// type-erased so any owning handle/client can be attached (H1 pool handle,
/// dedicated `MyHttpClient`, ssh client handle, …).
pub fn attach_conn_guard(
    resp: my_http_client::HyperResponse,
    guard: Box<dyn Send + Sync>,
) -> my_http_client::HyperResponse {
    let (parts, inner) = resp.into_parts();
    let guarded = GuardedBody {
        inner,
        _guard: guard,
    }
    .boxed();
    http::Response::from_parts(parts, guarded)
}
