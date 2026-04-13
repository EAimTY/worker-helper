use bytes::{Buf, Bytes};
use futures::TryStreamExt as _;
use http_body::{Body as HttpBody, Frame};
use http_body_util::BodyExt as _;
#[cfg(feature = "json")]
use serde_json::Error as JsonError;
#[cfg(feature = "yaml")]
use serde_yaml::Error as YamlError;
use std::{
    convert::Infallible,
    future::Future,
    pin::{Pin, pin},
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
/// Errors returned while collecting an HTTP body as text.
pub enum TextBodyError<E> {
    /// The underlying body returned an error while streaming frames.
    #[error(transparent)]
    Body(#[from] E),
    /// The collected body was not valid UTF-8.
    #[error("bad UTF-8 encoding")]
    BadUtf8Encoding,
}

#[cfg(feature = "json")]
#[derive(Debug, Error)]
/// Errors returned while collecting and decoding an HTTP body as JSON.
pub enum JsonBodyError<E> {
    /// The underlying body returned an error while streaming frames.
    #[error(transparent)]
    Body(#[from] E),
    /// The collected body could not be parsed as JSON.
    #[error("failed decoding body as JSON: {0}")]
    Decode(#[source] JsonError),
}

#[cfg(feature = "yaml")]
#[derive(Debug, Error)]
/// Errors returned while collecting and decoding an HTTP body as YAML.
pub enum YamlBodyError<E> {
    /// The underlying body returned an error while streaming frames.
    #[error(transparent)]
    Body(#[from] E),
    /// The collected body could not be parsed as YAML.
    #[error("failed decoding body as YAML: {0}")]
    Decode(#[source] YamlError),
}

/// Extension methods for receiving an HTTP body as text and, when enabled,
/// JSON or YAML.
pub trait BodyExt: HttpBody {
    /// Collects the streamed body into a UTF-8 string.
    ///
    /// This method correctly handles multi-byte UTF-8 sequences split across
    /// frame boundaries.
    fn text(self) -> impl Future<Output = Result<String, TextBodyError<Self::Error>>>;

    #[cfg(feature = "json")]
    /// Collects the body and deserializes it as JSON.
    ///
    /// Available with the `json` feature.
    fn json<T>(self) -> impl Future<Output = Result<T, JsonBodyError<Self::Error>>>
    where
        T: for<'a> Deserialize<'a>;

    #[cfg(feature = "yaml")]
    /// Collects the body and deserializes it as YAML.
    ///
    /// Available with the `yaml` feature.
    fn yaml<T>(self) -> impl Future<Output = Result<T, YamlBodyError<Self::Error>>>
    where
        T: for<'a> Deserialize<'a>;
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

impl<B> BodyExt for B
where
    B: HttpBody<Data = Bytes>,
{
    async fn text(self) -> Result<String, TextBodyError<Self::Error>> {
        let mut frames = pin!(self.into_data_stream());
        let mut text = String::new();
        let mut incomplete = None::<IncompleteUtf8>;

        while let Some(frame) = frames.try_next().await? {
            let mut frame = frame.as_ref();

            if let Some(current_incomplete) = incomplete.as_mut()
                && let Some((result, remaining)) = current_incomplete.try_complete(frame)
            {
                match result {
                    Ok(chunk) => text.push_str(chunk),
                    Err(_) => return Err(TextBodyError::BadUtf8Encoding),
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
                    return Err(TextBodyError::BadUtf8Encoding);
                }
            }
        }

        if incomplete.is_some() {
            return Err(TextBodyError::BadUtf8Encoding);
        }

        Ok(text)
    }

    #[cfg(feature = "json")]
    async fn json<T>(self) -> Result<T, JsonBodyError<Self::Error>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let frames = self.into_data_stream().try_collect::<Vec<_>>().await?;
        let reader = BytesSliceReader::new(frames.into_iter());
        serde_json::from_reader(reader).map_err(JsonBodyError::Decode)
    }

    #[cfg(feature = "yaml")]
    async fn yaml<T>(self) -> Result<T, YamlBodyError<Self::Error>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let frames = self.into_data_stream().try_collect::<Vec<_>>().await?;
        let reader = BytesSliceReader::new(frames.into_iter());
        serde_yaml::from_reader(reader).map_err(YamlBodyError::Decode)
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
