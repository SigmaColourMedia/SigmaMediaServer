use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes, BytesMut};
use crate::{Unmarshall, UnmarshallError};

#[derive(Debug, PartialEq)]
pub(crate) struct Header {
    pub(crate) payload_type: PayloadType,
    pub(crate) length: u16,
    pub(crate) feedback_message_type: u8,
    pub(crate) padding: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) enum PayloadType {
    ReceiverReport,
    TransportLayerFeedbackMessage,
    PayloadSpecificFeedbackMessage,
}

impl Unmarshall for Header {
    fn unmarshall(value: Bytes) -> Result<Self, UnmarshallError> {
        if value.len() < HEADER_LEN {
            return Err(UnmarshallError::InvalidLength);
        }

        let mut value = value.reader();
        let first_octet = value.read_u8().or(Err(UnmarshallError::UnexpectedFrame))?;

        let version = (VERSION_MASK & first_octet) >> VERSION_SHIFT;
        if version != RTCP_VERSION {
            return Err(UnmarshallError::UnexpectedFrame);
        }

        let padding = (PADDING_MASK & first_octet) >> PADDING_SHIFT == 1;

        let fb_message_type = FMT_MASK & first_octet;
        let payload_type = match value.read_u8().or(Err(UnmarshallError::UnexpectedFrame))? {
            205 => PayloadType::TransportLayerFeedbackMessage,
            206 => PayloadType::PayloadSpecificFeedbackMessage,
            201 => PayloadType::ReceiverReport,
            _ => return Err(UnmarshallError::UnexpectedFrame),
        };
        let length = value
            .read_u16::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;

        return Ok(Header {
            padding,
            length,
            feedback_message_type: fb_message_type,
            payload_type,
        });
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn unmarshall_ok() {
        let header_buffer = vec![0b1000_0001u8, 201, 0, 7];
        let bytes = Bytes::from(header_buffer);
        let header = Header::unmarshall(bytes).unwrap();

        assert_eq!(header, Header {
            length: 7,
            payload_type: PayloadType::ReceiverReport,
            padding: false,
            feedback_message_type: 1,
        });
    }

    #[test]
    fn unmarshall_reject_on_invalid_version() {
        let header_buffer = vec![0b1100_0001u8, 201, 0, 7];
        let bytes = Bytes::from(header_buffer);
        let header = Header::unmarshall(bytes);
        assert_eq!(header.unwrap_err(), UnmarshallError::UnexpectedFrame)
    }

    #[test]
    fn unmarshall_reject_on_invalid_length() {
        let header_buffer = vec![0b1000_0001u8, 201, 0];
        let bytes = Bytes::from(header_buffer);
        let header = Header::unmarshall(bytes);
        assert_eq!(header.unwrap_err(), UnmarshallError::InvalidLength)
    }
}

static HEADER_LEN: usize = 4;
static VERSION_MASK: u8 = 0b1100_0000;
static RTCP_VERSION: u8 = 2;
static VERSION_SHIFT: u8 = 6;
static PADDING_MASK: u8 = 0b0010_0000;
static PADDING_SHIFT: u8 = 5;
static FMT_MASK: u8 = 0b0001_1111;
