use bytes::Bytes;
use http_body::{Body as BodyTrait, Frame};
use http_body_util::Full;
use serde::Serialize;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

/// An in-memory YAML body built from a serializable value.
///
/// Available with the `yaml` feature.
pub struct Yaml(Full<Bytes>);

impl Yaml {
    /// Serializes `yaml` into a full body.
    ///
    /// # Panics
    ///
    /// Panics if `yaml` cannot be serialized as YAML.
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
