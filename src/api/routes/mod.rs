use bytes::Bytes;
use http_body_util::Full;
use hyper::Response;

pub mod error;
pub mod thumbnail;
pub mod whep;
pub mod whip;

pub type RouteResult = Result<Response<Full<Bytes>>, hyper::Error>;
pub type HTTPResponse = Response<Full<Bytes>>;
