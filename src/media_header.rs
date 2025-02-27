use bytes::{Buf, Bytes};
use rtcp::{Marshall, Unmarshall, UnmarshallError};
use rtcp;

#[derive(Debug, Clone, PartialEq)]
enum MediaHeader {
    RTP(RTPHeader),
    RTCP(rtcp::header::Header),
}


#[derive(Debug, Clone, PartialEq)]
struct RTPHeader {
    padding: bool,
    extension: bool,
    marker: bool,
    seq: u16,
    payload_type: u8,
    timestamp: u32,
    ssrc: u32,
    csrc_identifiers: Vec<u32>,
}


impl Unmarshall for MediaHeader {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let payload_type = bytes.slice(1..).get_u8();

        match payload_type {
            // Supported RTCP PT range
            199..=210 =>
                Ok(MediaHeader::RTCP(rtcp::header::Header::unmarshall(bytes)?)),
            // Assume rest is RTP PT range
            _ => { Ok(MediaHeader::RTP(RTPHeader::unmarshall(bytes)?)) }
        }
    }
}


impl Unmarshall for RTPHeader {
    fn unmarshall(mut bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let first_octet = bytes.get_u8();
        let version = (first_octet & VERSION_MASK) >> VERSION_SHIFT;

        if version != 2 {
            return Err(UnmarshallError::UnexpectedFrame);
        }

        let is_padding_set = ((first_octet & PADDING_MASK) >> PADDING_SHIFT) == 1;
        let is_extension = ((first_octet & EXTENSION_MASK) >> EXTENSION_SHIFT) == 1;
        let csrc_count = first_octet & CSRC_COUNT_MASK;

        let second_octet = bytes.get_u8();

        let is_market_set = ((second_octet & MARKER_MASK) >> MARKER_SHIFT) == 1;
        let payload_type = second_octet & PAYLOAD_TYPE_MASK;

        let seq_number = bytes.get_u16();
        let timestamp = bytes.get_u32();
        let ssrc = bytes.get_u32();

        let mut csrc_identifiers = vec![];

        for _ in 0..csrc_count {
            csrc_identifiers.push(bytes.get_u32())
        }


        Ok(Self {
            seq: seq_number,
            timestamp,
            marker: is_market_set,
            extension: is_extension,
            padding: is_padding_set,
            csrc_identifiers,
            payload_type,
            ssrc,
        })
    }
}


static VERSION_MASK: u8 = 0b1100_0000;
static VERSION_SHIFT: u8 = 6;
static PADDING_MASK: u8 = 0b0010_0000;
static PADDING_SHIFT: u8 = 5;
static EXTENSION_MASK: u8 = 0b0001_0000;
static EXTENSION_SHIFT: u8 = 4;
static CSRC_COUNT_MASK: u8 = 0b0000_1111;
static MARKER_MASK: u8 = 0b1000_0000;
static MARKER_SHIFT: u8 = 7;
static PAYLOAD_TYPE_MASK: u8 = 0b0111_1111;

#[cfg(test)]
mod unmarshall_media_header {
    use bytes::Bytes;
    use rtcp::header::PayloadType;
    use rtcp::Unmarshall;
    use crate::media_header::{MediaHeader, RTPHeader};

    #[test]
    fn unmarshall_rtcp_packet() {
        let input = Bytes::from_static(&
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
            1, 0, 0, 2 // Generic NACK
        ]);
        let actual_output = MediaHeader::unmarshall(input).unwrap();

        assert_eq!(actual_output, MediaHeader::RTCP(rtcp::header::Header {
            padding: false,
            length: 3,
            payload_type: PayloadType::TransportLayerFeedbackMessage,
            feedback_message_type: 1,
        }))
    }

    #[test]
    fn unmarshall_rtp_packet() {
        let input = Bytes::from_static(&[
            128, 111, 2, 0, // Version = 2, no padding, no extension, no CSRC, payload_type = 111, sequence_number = 512
            0, 0, 0, 20, // Timestamp = 20
            0, 0, 0, 1, // SSRC = 1
        ]);

        let actual_output = MediaHeader::unmarshall(input).unwrap();

        assert_eq!(actual_output, MediaHeader::RTP(RTPHeader {
            ssrc: 1,
            payload_type: 111,
            padding: false,
            seq: 512,
            marker: false,
            extension: false,
            csrc_identifiers: vec![],
            timestamp: 20,
        }));
    }
}


#[cfg(test)]
mod unmarshall_rtp_header {
    use bytes::Bytes;
    use rtcp::Unmarshall;
    use crate::media_header::RTPHeader;

    #[test]
    fn unmarshall_rtp_with_no_csrc_ident() {
        let input = Bytes::from_static(&[
            128, 111, 2, 0, // Version = 2, no padding, no extension, no CSRC, payload_type = 111, sequence_number = 512
            0, 0, 0, 20, // Timestamp = 20
            0, 0, 0, 1, // SSRC = 1
        ]);

        let actual_output = RTPHeader::unmarshall(input).unwrap();

        let expected_output = RTPHeader {
            csrc_identifiers: vec![],
            padding: false,
            timestamp: 20,
            ssrc: 1,
            extension: false,
            marker: false,
            seq: 512,
            payload_type: 111,
        };


        assert_eq!(actual_output, expected_output)
    }

    #[test]
    fn unmarshall_rtp_with_two_csrc_ident() {
        let input = Bytes::from_static(&[
            130, 111, 2, 0, // Version = 2, no padding, no extension, CSRC_count = 2, payload_type = 111, sequence_number = 512
            0, 0, 0, 20, // Timestamp = 20
            0, 0, 0, 1, // SSRC = 1
            0, 0, 0, 5, // CSRC no. 1 = 5
            0, 0, 0, 7 // CSRC no. 2 = 7
        ]);

        let actual_output = RTPHeader::unmarshall(input).unwrap();

        let expected_output = RTPHeader {
            csrc_identifiers: vec![5, 7],
            padding: false,
            timestamp: 20,
            ssrc: 1,
            extension: false,
            marker: false,
            seq: 512,
            payload_type: 111,
        };


        assert_eq!(actual_output, expected_output)
    }
}