mod header;
mod transport_layer_feedback;
mod payload_specific_feedback;
mod rtcp;

use byteorder::{ReadBytesExt};
use std::io::{Read, Seek};
use bytes::Bytes;


trait Unmarshall {
    fn unmarshall(bytes: bytes::Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized;
}

trait Marshall {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq)]
enum UnmarshallError {
    UnexpectedFrame,
    InvalidLength,
}

#[derive(Debug, PartialEq)]
enum MarshallError {
    UnexpectedFrame
}


static NACK_FMT: usize = 1;

static HEADER_LEN: usize = 4;
static SELF_SSR_LEN: usize = 4;
