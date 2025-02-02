use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::{Marshall, MarshallError, Unmarshall, UnmarshallError};

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Header {
    pub(crate) payload_type: PayloadType,
    pub(crate) length: u16,
    pub(crate) feedback_message_type: u8,
    pub(crate) padding: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum PayloadType {
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

impl Marshall for Header {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        let version = 0b1000_0000;
        let padding = if self.padding { 0b0010_0000 } else { 0b0000_0000 };
        let first_octet = version | padding | self.feedback_message_type;
        let second_octet = match &self.payload_type {
            PayloadType::TransportLayerFeedbackMessage => TRANSPORT_LAYER_PT,
            PayloadType::PayloadSpecificFeedbackMessage => PAYLOAD_SPECIFIC_PT,
        };
        bytes.put_u8(first_octet);
        bytes.put_u8(second_octet);
        bytes.put_u16(self.length);

        Ok(bytes.freeze())
    }
}

#[cfg(test)]
mod marshall_tests {
    use super::*;

    #[test]
    fn marshall_ok_ps_fb() {
        let input = Header {
            length: 2,
            payload_type: PayloadType::PayloadSpecificFeedbackMessage,
            padding: false,
            feedback_message_type: 1,
        };


        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            129, 206, 0, 2
        ]))
    }

    #[test]
    fn marshall_ok_tl_fb() {
        let input = Header {
            length: 256,
            payload_type: PayloadType::TransportLayerFeedbackMessage,
            padding: false,
            feedback_message_type: 1,
        };


        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            129, 205, 1, 0
        ]))
    }

    #[test]
    fn marshall_ok_with_padding() {
        let input = Header {
            length: 2,
            payload_type: PayloadType::TransportLayerFeedbackMessage,
            padding: true,
            feedback_message_type: 1,
        };


        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            161, 205, 0, 2
        ]))
    }
}

#[cfg(test)]
mod unmarshall_tests {
    use super::*;


    #[test]
    fn unmarshall_ok() {
        let header_buffer = vec![0b1000_0001u8, 206, 0, 7];
        let bytes = Bytes::from(header_buffer);
        let header = Header::unmarshall(bytes).unwrap();

        assert_eq!(header, Header {
            length: 7,
            payload_type: PayloadType::PayloadSpecificFeedbackMessage,
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
static PAYLOAD_SPECIFIC_PT: u8 = 206;
static TRANSPORT_LAYER_PT: u8 = 205;
