use bytes::Bytes;
use http_body::{Body as BodyTrait, Frame};
use http_body_util::Full;
use serde::Serialize;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use worker::Error;

/// A Json Value
pub struct Json(Full<Bytes>);

impl Json {
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
    type Error = Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0)
            .poll_frame(cx)
            .map_err(|_| unreachable!())
    }
}
