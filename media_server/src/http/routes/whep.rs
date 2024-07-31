use std::sync::mpsc::Sender;

use crate::http::{HttpError, HTTPMethod, Request, Response, ServerCommand};
use crate::http::parsers::map_http_err_to_response;

pub fn whep_route(request: Request, command_sender: Sender<ServerCommand>) -> Response {
    match &request.method {
        HTTPMethod::GET => {
            register_viewer(request, command_sender).unwrap_or_else(map_http_err_to_response)
        }
        _ => map_http_err_to_response(HttpError::MethodNotAllowed),
    }
}

fn register_viewer(
    request: Request,
    command_sender: Sender<ServerCommand>,
) -> Result<Response, HttpError> {
    todo!()
    // let target_id = request
    //     .search
    //     .get("target_id")
    //     .ok_or(HttpError::BadRequest)?;
    //
    // let (tx, rx) = std::sync::mpsc::channel::<Option<SDP>>();
    //
    // command_sender
    //     .send(ServerCommand::GetStreamSDP((tx, target_id.clone())))
    //     .unwrap();
    //
    // let stream_sdp = rx.recv().unwrap().ok_or(HttpError::NotFound)?;
    // let (sdp_answer, credentials) =
    //     create_streaming_sdp_answer(&stream_sdp).ok_or(HttpError::BadRequest)?;
    //
    // let viewer_session = Session::new_viewer(target_id.to_owned(), credentials);
    //
    // let response_builder = ResponseBuilder::new();
    // let response = response_builder
    //     .set_status(200)
    //     .set_header("content-type", "application/sdp")
    //     .set_header("Access-Control-Allow-Methods", "GET")
    //     .set_header("Access-Control-Allow-Origin", "http://localhost:9000")
    //     .set_header("location", "http://localhost:8080/whep")
    //     .set_body(sdp_answer.as_bytes())
    //     .build();
    //
    // command_sender
    //     .send(ServerCommand::AddViewer(viewer_session))
    //     .unwrap();
    //
    // Ok(response)
}
