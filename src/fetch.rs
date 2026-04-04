use crate::body::Body;
use bytes::Bytes;
use http::{Request, Response};
use http_body::Body as HttpBody;
use worker::{Error, Fetch as RawFetch};

/// Sends an [`http::Request`] with `worker::Fetch` and returns a response body
/// wrapped in [`crate::Body`].
///
/// This is useful when the rest of your code already works with
/// `http::Request<T>` and `http_body::Body` instead of the `worker` crate's
/// request types.
pub struct Fetch<T>(pub Request<T>);

impl<T> Fetch<T>
where
    T: HttpBody<Data = Bytes> + 'static,
{
    /// Sends the request and converts the response body into [`crate::Body`].
    pub async fn send(self) -> Result<Response<Body<Error>>, Error> {
        let request = self.0.try_into()?;
        let response = RawFetch::Request(request).send().await?;
        let (parts, body) = Response::try_from(response)?.into_parts();
        Ok(Response::from_parts(parts, Body::new(body)))
    }
}
