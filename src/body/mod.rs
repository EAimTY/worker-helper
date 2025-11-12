use bytes::Bytes;
use futures::TryStreamExt as _;
use http_body::{Body as BodyTrait, Frame};
use http_body_util::{BodyExt as _, combinators::BoxBody};
use serde::Deserialize;
use slice_of_bytes_reader::Reader as BytesSliceReader;
use std::{
    io::Error as IoError,
    pin::Pin,
    task::{Context, Poll},
};
use utf8::{DecodeError as Utf8DecodeError, Incomplete as IncompleteUtf8};
use worker::Error;

mod json;
mod yaml;

pub use self::{json::Json, yaml::Yaml};

/// Universal request / response body
pub struct Body(BoxBody<Bytes, Error>);

impl Body {
    pub fn new<T>(body: T) -> Self
    where
        T: BodyTrait<Data = Bytes, Error = Error> + Send + Sync + 'static,
    {
        Self(BoxBody::new(body))
    }

    pub async fn text(self) -> Result<String, Error> {
        let mut frames = self.0.into_data_stream();
        let mut text = String::new();
        let mut incomplete = None::<IncompleteUtf8>;

        while let Some(frame) = frames.try_next().await? {
            let mut frame = frame.as_ref();

            if let Some(current_incomplete) = incomplete.as_mut()
                && let Some((result, remaining)) = current_incomplete.try_complete(frame)
            {
                match result {
                    Ok(chunk) => text.push_str(chunk),
                    Err(_) => return Err(Error::BadEncoding),
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
                Err(Utf8DecodeError::Invalid { .. }) => return Err(Error::BadEncoding),
            }
        }

        if incomplete.is_some() {
            return Err(Error::BadEncoding);
        }

        Ok(text)
    }

    pub async fn json<T>(self) -> Result<T, Error>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut frames = self.0.into_data_stream();
        let mut chunks = Vec::new();

        while let Some(chunk) = frames.try_next().await? {
            chunks.push(chunk);
        }

        let reader = BytesSliceReader::new(chunks.into_iter());
        serde_json::from_reader(reader).map_err(|err| Error::from(IoError::other(err)))
    }

    pub async fn yaml<T>(self) -> Result<T, Error>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut frames = self.0.into_data_stream();
        let mut chunks = Vec::new();

        while let Some(chunk) = frames.try_next().await? {
            chunks.push(chunk);
        }

        let reader = BytesSliceReader::new(chunks.into_iter());
        serde_yaml::from_reader(reader).map_err(|err| Error::from(IoError::other(err)))
    }
}

impl BodyTrait for Body {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }
}
