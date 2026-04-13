use bytes::Bytes;
use http::{Request, Response};
use http_body::Body as HttpBody;
use worker::{Body as WorkerBody, Error, Fetch as RawFetch};

/// Sends an [`http::Request`] with `worker::Fetch` and returns a response body
/// compatible with [`crate::body::BodyExt`].
///
/// This is useful when the rest of your code already works with
/// `http::Request<T>` and `http_body::Body` instead of the `worker` crate's
/// request types.
pub struct Fetch<T>(pub Request<T>);

impl<T> Fetch<T>
where
    T: HttpBody<Data = Bytes> + 'static,
{
    /// Sends the request and returns the response as `http::Response<worker::Body>`.
    pub async fn send(self) -> Result<Response<WorkerBody>, Error> {
        let request = self.0.try_into()?;
        let response = RawFetch::Request(request).send().await?;
        Response::try_from(response)
    }
}
