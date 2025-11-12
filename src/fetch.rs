use crate::http::body::util::HelperBody;
use bytes::Bytes;
use http::{Request, Response};
use http_body::Body;
use worker::{Error, Fetch as RawFetch};

pub struct Fetch<T>(pub Request<T>);

impl<T> Fetch<T>
where
    T: Body<Data = Bytes> + 'static,
{
    pub async fn send(self) -> Result<Response<HelperBody<Error>>, Error> {
        let request = self.0.try_into()?;
        let response = RawFetch::Request(request).send().await?;
        let (parts, body) = Response::try_from(response)?.into_parts();
        Ok(Response::from_parts(parts, HelperBody::new(body)))
    }
}
