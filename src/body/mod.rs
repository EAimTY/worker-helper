use bytes::{Buf, Bytes};
use futures::TryStreamExt as _;
use http_body::{Body as BodyTrait, Frame};
use http_body_util::{BodyExt as _, combinators::BoxBody};
use serde::Deserialize;
use serde_json::Error as JsonError;
use serde_yaml::Error as YamlError;
use slice_of_bytes_reader::Reader as BytesSliceReader;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use thiserror::Error;
use utf8::{DecodeError as Utf8DecodeError, Incomplete as IncompleteUtf8};

mod json;
mod yaml;

pub use self::{json::Json, yaml::Yaml};

/// Universal request / response body
pub struct Body<E>(BoxBody<Bytes, E>);

pub struct MapErrorBody<B, E1, E2>
where
    B: BodyTrait<Error = E1>,
{
    body: B,
    map_error_fn: fn(E1) -> E2,
}

#[derive(Debug, Error)]
pub enum ReceiveBodyError<E> {
    #[error(transparent)]
    Receive(E),
    #[error("bad UTF-8 encoding")]
    BadUtf8Encoding,
    #[error(transparent)]
    InvalidJson(JsonError),
    #[error(transparent)]
    InvalidYaml(YamlError),
}

impl<E> Body<E> {
    pub fn new<T>(body: T) -> Self
    where
        T: BodyTrait<Data = Bytes, Error = E> + Send + Sync + 'static,
    {
        Self(BoxBody::new(body))
    }

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

    pub async fn json<T>(self) -> Result<T, ReceiveBodyError<E>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut frames = self.0.into_data_stream();
        let mut chunks = Vec::new();

        while let Some(chunk) = frames.try_next().await.map_err(ReceiveBodyError::Receive)? {
            chunks.push(chunk);
        }

        let reader = BytesSliceReader::new(chunks.into_iter());
        serde_json::from_reader(reader).map_err(ReceiveBodyError::InvalidJson)
    }

    pub async fn yaml<T>(self) -> Result<T, ReceiveBodyError<E>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut frames = self.0.into_data_stream();
        let mut chunks = Vec::new();

        while let Some(chunk) = frames.try_next().await.map_err(ReceiveBodyError::Receive)? {
            chunks.push(chunk);
        }

        let reader = BytesSliceReader::new(chunks.into_iter());
        serde_yaml::from_reader(reader).map_err(ReceiveBodyError::InvalidYaml)
    }
}

impl<B, E1, E2> MapErrorBody<B, E1, E2>
where
    B: BodyTrait<Error = E1>,
{
    pub fn new(body: B, map_error_fn: fn(E1) -> E2) -> Self {
        Self { body, map_error_fn }
    }
}

impl<E> BodyTrait for Body<E> {
    type Data = Bytes;
    type Error = E;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }
}

impl<B, D, E1, E2> BodyTrait for MapErrorBody<B, E1, E2>
where
    B: BodyTrait<Data = D, Error = E1> + Unpin,
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
