use bytes::Bytes;
use http_body::{Body as BodyTrait, Frame};
use http_body_util::Full;
use serde::Serialize;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

/// A YAML Value
pub struct Yaml(Full<Bytes>);

impl Yaml {
    pub fn new<T>(yaml: T) -> Self
    where
        T: Serialize,
    {
        let body = serde_yaml::to_string(&yaml).unwrap();
        Self(Full::new(Bytes::from(body)))
    }
}

impl BodyTrait for Yaml {
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
