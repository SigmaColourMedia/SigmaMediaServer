use crate::http::parsers::map_http_err_to_response;
use crate::http::server_builder::Context;
use crate::http::{HTTPMethod, HttpError, Request};

pub async fn whep_route(request: Request, context: Context) -> String {
    match &request.method {
        HTTPMethod::GET => {}
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}
