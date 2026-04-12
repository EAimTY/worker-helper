use bytes::{Buf, Bytes};
use futures::TryStreamExt as _;
use http_body::{Body as HttpBody, Frame};
use http_body_util::{BodyExt as _, combinators::BoxBody};
#[cfg(feature = "json")]
use serde_json::Error as JsonError;
#[cfg(feature = "yaml")]
use serde_yaml::Error as YamlError;
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};
use thiserror::Error;
use utf8::{DecodeError as Utf8DecodeError, Incomplete as IncompleteUtf8};
#[cfg(any(feature = "json", feature = "yaml"))]
use {serde::Deserialize, slice_of_bytes_reader::Reader as BytesSliceReader};

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "yaml")]
mod yaml;

#[cfg(feature = "json")]
pub use self::json::Json;
#[cfg(feature = "yaml")]
pub use self::yaml::Yaml;

/// A boxed HTTP body with convenience methods for collecting text and, when
/// enabled, JSON or YAML payloads.
pub struct Body<E>(BoxBody<Bytes, E>);

/// A body adapter that maps the underlying body's error type.
pub struct MapErrorBody<B, E1, E2>
where
    B: HttpBody<Error = E1>,
{
    body: B,
    map_error_fn: fn(E1) -> E2,
}

/// A convenience adapter for bodies whose error type is [`Infallible`].
pub struct MapInfallibleErrorBody<B, E>(MapErrorBody<B, Infallible, E>)
where
    B: HttpBody<Error = Infallible>;

#[derive(Debug, Error)]
/// Errors returned while collecting or decoding a [`Body`].
pub enum ReceiveBodyError<E> {
    /// The underlying body returned an error while streaming frames.
    #[error(transparent)]
    Receive(E),
    /// The collected body was not valid UTF-8.
    #[error("bad UTF-8 encoding")]
    BadUtf8Encoding,
    /// The collected body could not be parsed as JSON.
    #[cfg(feature = "json")]
    #[error(transparent)]
    InvalidJson(JsonError),
    /// The collected body could not be parsed as YAML.
    #[cfg(feature = "yaml")]
    #[error(transparent)]
    InvalidYaml(YamlError),
}

impl<E> Body<E> {
    /// Boxes any compatible HTTP body so it can be handled uniformly.
    pub fn new<T>(body: T) -> Self
    where
        T: HttpBody<Data = Bytes, Error = E> + Send + Sync + 'static,
    {
        Self(BoxBody::new(body))
    }

    /// Collects the streamed body into a UTF-8 string.
    ///
    /// This method correctly handles multi-byte UTF-8 sequences split across
    /// frame boundaries.
    pub async fn text(self) -> Result<String, ReceiveBodyError<E>> {
        let mut frames = self.0.into_data_stream();
        let mut text = String::new();
        let mut incomplete = None::<IncompleteUtf8>;

        while let Some(frame) = frames.try_next().await.map_err(ReceiveBodyError::Receive)? {
            let mut frame = frame.as_ref();

            if let Some(current_incomplete) = incomplete.as_mut()
                && let Some((result, remaining)) = current_incomplete.try_complete(frame)
            {
                match result {
                    Ok(chunk) => text.push_str(chunk),
                    Err(_) => return Err(ReceiveBodyError::BadUtf8Encoding),
                }

                incomplete = None;
                frame = remaining;
            }

            if frame.is_empty() {
                continue;
            }

            match utf8::decode(frame) {
                Ok(chunk) => text.push_str(chunk),
                Err(Utf8DecodeError::Incomplete {
                    valid_prefix,
                    incomplete_suffix,
                }) => {
                    text.push_str(valid_prefix);
                    incomplete = Some(incomplete_suffix);
                }
                Err(Utf8DecodeError::Invalid { .. }) => {
                    return Err(ReceiveBodyError::BadUtf8Encoding);
                }
            }
        }

        if incomplete.is_some() {
            return Err(ReceiveBodyError::BadUtf8Encoding);
        }

        Ok(text)
    }

    #[cfg(feature = "json")]
    /// Collects the body and deserializes it as JSON.
    ///
    /// Available with the `json` feature.
    pub async fn json<T>(self) -> Result<T, ReceiveBodyError<E>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let frames = self
            .0
            .into_data_stream()
            .try_collect::<Vec<_>>()
            .await
            .map_err(ReceiveBodyError::Receive)?;

        let reader = BytesSliceReader::new(frames.into_iter());
        serde_json::from_reader(reader).map_err(ReceiveBodyError::InvalidJson)
    }

    #[cfg(feature = "yaml")]
    /// Collects the body and deserializes it as YAML.
    ///
    /// Available with the `yaml` feature.
    pub async fn yaml<T>(self) -> Result<T, ReceiveBodyError<E>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let frames = self
            .0
            .into_data_stream()
            .try_collect::<Vec<_>>()
            .await
            .map_err(ReceiveBodyError::Receive)?;

        let reader = BytesSliceReader::new(frames.into_iter());
        serde_yaml::from_reader(reader).map_err(ReceiveBodyError::InvalidYaml)
    }
}

impl<B, E1, E2> MapErrorBody<B, E1, E2>
where
    B: HttpBody<Error = E1>,
{
    /// Wraps `body` and maps each body error through `map_error_fn`.
    pub fn new(body: B, map_error_fn: fn(E1) -> E2) -> Self {
        Self { body, map_error_fn }
    }
}

impl<B, E> MapInfallibleErrorBody<B, E>
where
    B: HttpBody<Error = Infallible>,
{
    /// Wraps an infallible body and exposes it as a body with any error type.
    pub fn new(body: B) -> Self {
        Self(MapErrorBody::new(body, |_| unreachable!()))
    }
}

impl<E> HttpBody for Body<E> {
    type Data = Bytes;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }
}

impl<B, D, E1, E2> HttpBody for MapErrorBody<B, E1, E2>
where
    B: HttpBody<Data = D, Error = E1> + Unpin,
    D: Buf,
{
    type Data = D;
    type Error = E2;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.body)
            .poll_frame(cx)
            .map_err(|err| (self.map_error_fn)(err))
    }
}

impl<B, D, E> HttpBody for MapInfallibleErrorBody<B, E>
where
    B: HttpBody<Data = D, Error = Infallible> + Unpin,
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }
}
