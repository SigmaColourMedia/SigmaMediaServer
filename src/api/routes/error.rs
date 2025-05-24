use bytes::Bytes;
use http_body_util::Full;
use hyper::Response;

use crate::api::HTTPError;
use crate::api::routes::RouteResult;

pub async fn error_route(http_error: HTTPError) -> RouteResult {
    match http_error {
        HTTPError::NotFound => Ok(Response::builder()
            .status(404)
            .body(Full::new(Bytes::from("404 Not Found")))
            .unwrap()),
        HTTPError::BadRequest => Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("400 Bad Request")))
            .unwrap()),
        HTTPError::NotAuthorized => Ok(Response::builder()
            .status(403)
            .body(Full::new(Bytes::from("403 Not Authorized")))
            .unwrap()),
        HTTPError::ServerError => Ok(Response::builder()
            .status(500)
            .body(Full::new(Bytes::from("500 Server Error")))
            .unwrap()),
    }
}
