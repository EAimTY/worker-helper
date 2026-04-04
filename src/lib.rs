#![doc = include_str!("../README.md")]

mod body;
mod fetch;

pub use crate::{
    body::{Body, MapErrorBody, MapInfallibleErrorBody, ReceiveBodyError},
    fetch::Fetch,
};

#[rustfmt::skip]
#[cfg(feature = "json")]
pub use crate::body::Json;

#[rustfmt::skip]
#[cfg(feature = "yaml")]
pub use crate::body::Yaml;
