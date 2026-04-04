use bytes::Bytes;
use http_body::{Body as BodyTrait, Frame};
use http_body_util::Full;
use serde::Serialize;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

/// An in-memory JSON body built from a serializable value.
///
/// Available with the `json` feature.
pub struct Json(Full<Bytes>);

impl Json {
    /// Serializes `json` into a full body.
    ///
    /// # Panics
    ///
    /// Panics if `json` cannot be serialized as JSON.
    pub fn new<T>(json: T) -> Self
    where
        T: Serialize,
    {
        let body = serde_json::to_vec(&json).unwrap();
        Self(Full::new(Bytes::from(body)))
    }
}

impl BodyTrait for Json {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0)
            .poll_frame(cx)
            .map_err(|_| unreachable!())
    }
}
