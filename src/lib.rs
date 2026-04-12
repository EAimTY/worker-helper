#![doc = include_str!("../README.md")]

pub mod body;
pub mod fetch;

pub use crate::{body::Body, fetch::Fetch};
