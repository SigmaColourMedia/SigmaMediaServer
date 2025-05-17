mod routes;
pub mod server;

pub enum HTTPError {
    NotFound,
    BadRequest,
    NotAuthorized,
    ServerError,
}
