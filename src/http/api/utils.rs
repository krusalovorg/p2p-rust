use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use std::convert::Infallible;
use http_body_util::BodyExt;

pub const SHOW_LOGS: bool = false;

pub fn log(message: &str) {
    if SHOW_LOGS {
        println!("{}", message);
    }
}

pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).map_err(|_| unreachable!()).boxed()
} 