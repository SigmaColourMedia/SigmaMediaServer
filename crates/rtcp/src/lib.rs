mod header;
mod transport_layer_feedback;
mod payload_specific_feedback;

use byteorder::{ReadBytesExt};
use std::io::{Read, Seek};
use crate::header::{Header, PayloadType};
use crate::payload_specific_feedback::PayloadSpecificFeedback;
use crate::transport_layer_feedback::TransportLayerNACK;


trait Unmarshall {
    fn unmarshall(bytes: bytes::Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq)]
enum UnmarshallError {
    UnexpectedFrame,
    InvalidLength,
}


struct ReceiverReport {
    report_count: u8,
    length: u16,
    sender_ssrc: u32,
    padding: bool,
    report_blocks: Vec<ReportBlock>,
}

struct ReportBlock {
    fraction_lost: u8,
    packets_lost_count: u32,
    last_seq_number: u32,
    jitter: u32,
    lsr: u32,
    dlsr: u32,
}


#[derive(Debug, PartialEq)]
enum RtcpPacket {
    TransportLayerFeedbackMessage(TransportLayerNACK),
    PayloadSpecificFeedbackMessage(PayloadSpecificFeedback),
}

impl Unmarshall for RtcpPacket {
    fn unmarshall(bytes: bytes::Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes.clone())?;

        match &header.payload_type {
            PayloadType::TransportLayerFeedbackMessage =>
                Ok(RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK::unmarshall(bytes)?)),

            PayloadType::PayloadSpecificFeedbackMessage => Ok(RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::unmarshall(bytes)?)),
            _ => Err(UnmarshallError::UnexpectedFrame)
        }
    }
}

#[cfg(test)]
mod rtcp_tests {
    use crate::transport_layer_feedback::GenericNACK;
    use super::*;


    #[test]
    fn rtcp_tl_nack_ok() {
        let bytes = bytes::Bytes::from_static(&
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
            1, 0, 0, 2 // Generic NACK
        ]);
        let rtcp_packet = RtcpPacket::unmarshall(bytes).unwrap();

        assert_eq!(rtcp_packet, RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK {
            sender_ssrc: 1,
            media_ssrc: 2,
            nacks: vec![GenericNACK {
                pid: 256,
                blp: 2,
            }],
        }))
    }

    #[test]
    fn rtcp_pli_ok() {
        let bytes = bytes::Bytes::from_static(&
        [129, 206, 0, 2, // Feedback Specific Layer Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
        ]);
        let rtcp_packet = RtcpPacket::unmarshall(bytes).unwrap();

        assert_eq!(rtcp_packet, RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::PictureLossIndication))
    }
}

static NACK_FMT: usize = 1;

static HEADER_LEN: usize = 4;
static SELF_SSR_LEN: usize = 4;
