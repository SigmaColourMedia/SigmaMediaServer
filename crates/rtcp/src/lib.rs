pub mod header;
pub mod transport_layer_feedback;
pub mod payload_specific_feedback;
pub mod rtcp;
pub mod receiver_report;
pub mod sender_report;
pub mod sdes;

use byteorder::{ReadBytesExt};
use std::io::{Read, Seek};
use bytes::{Buf, Bytes};
use crate::header::Header;
use crate::rtcp::RtcpPacket;


pub trait Unmarshall {
    fn unmarshall(bytes: bytes::Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized;
}

pub trait Marshall {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq)]
pub enum UnmarshallError {
    UnexpectedFrame,
    InvalidLength,
}

#[derive(Debug, PartialEq)]
pub enum MarshallError {
    UnexpectedFrame,
    UnsupportedFormat,
    InvalidLength,
}


pub fn unmarshall_compound_rtcp(input: Bytes) -> Result<Vec<RtcpPacket>, UnmarshallError> {
    let mut input = input;
    let mut packets: Vec<RtcpPacket> = vec![];
    while input.has_remaining() {
        let header = Header::unmarshall(input.clone())?;
        let length_to_slice = (header.length as usize * 4) + 4;

        let packet_buffer = input.slice(..length_to_slice);
        let packet = RtcpPacket::unmarshall(packet_buffer);
        if packet.is_ok() {
            packets.push(packet.unwrap())
        }
        input = input.slice(length_to_slice..);
    }

    Ok(packets)
}


#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use crate::header::{Header, PayloadType};
    use crate::payload_specific_feedback::{PayloadSpecificFeedback, PictureLossIndication};
    use crate::rtcp::RtcpPacket;
    use crate::transport_layer_feedback::{GenericNACK, TransportLayerNACK};
    use crate::unmarshall_compound_rtcp;

    #[test]
    fn unmarshalls_one_packet_in_compound() {
        let input = Bytes::from_static(&
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1,  // Sender ssrc = 1
            0, 0, 0, 2,  // Media ssrc = 2
            1, 0, 0, 2   // Generic NACK
        ]);

        let output = unmarshall_compound_rtcp(input).unwrap();

        assert_eq!(output, vec![RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK {
            header: Header {
                padding: false,
                length: 3,
                feedback_message_type: 1,
                payload_type: PayloadType::TransportLayerFeedbackMessage,
            },
            nacks: vec![GenericNACK { pid: 256, blp: 2 }],
            sender_ssrc: 1,
            media_ssrc: 2,
        })])
    }

    #[test]
    fn unmarshalls_two_packets_in_compound() {
        let input = Bytes::from_static(&
        // First Packet
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1,  // Sender ssrc = 1
            0, 0, 0, 2,  // Media ssrc = 2
            1, 0, 0, 2,   // Generic NACK
            // Second Packet
            129, 206, 0, 2, // Payload Specific PLI Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 2 // Media SSRC = 2
        ]);

        let output = unmarshall_compound_rtcp(input).unwrap();

        assert_eq!(output, vec![
            RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK {
                header: Header {
                    padding: false,
                    length: 3,
                    feedback_message_type: 1,
                    payload_type: PayloadType::TransportLayerFeedbackMessage,
                },
                nacks: vec![GenericNACK { pid: 256, blp: 2 }],
                sender_ssrc: 1,
                media_ssrc: 2,
            }),
            RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication {
                sender_ssrc: 1,
                media_ssrc: 2,
                header: Header {
                    padding: false,
                    length: 2,
                    payload_type: PayloadType::PayloadSpecificFeedbackMessage,
                    feedback_message_type: 1,
                },
            }))])
    }

    #[test]
    fn skips_unsupported_rtcp_packet() {
        let input = Bytes::from_static(&
        // First Packet
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1,  // Sender ssrc = 1
            0, 0, 0, 2,  // Media ssrc = 2
            1, 0, 0, 2,   // Generic NACK
            // Unsupported RTCP packet
            129, 220, 0, 0,
            // Second Packet
            129, 206, 0, 2, // Payload Specific PLI Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 2 // Media SSRC = 2
        ]);

        let output = unmarshall_compound_rtcp(input).unwrap();

        assert_eq!(output, vec![
            RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK {
                header: Header {
                    padding: false,
                    length: 3,
                    feedback_message_type: 1,
                    payload_type: PayloadType::TransportLayerFeedbackMessage,
                },
                nacks: vec![GenericNACK { pid: 256, blp: 2 }],
                sender_ssrc: 1,
                media_ssrc: 2,
            }),
            RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication {
                sender_ssrc: 1,
                media_ssrc: 2,
                header: Header {
                    padding: false,
                    length: 2,
                    payload_type: PayloadType::PayloadSpecificFeedbackMessage,
                    feedback_message_type: 1,
                },
            }))])
    }

    #[test]
    fn returns_empty_array_for_unsupported_compound() {
        let input = Bytes::from_static(&
        [
            129, 219, 0, 0,   // First unsupported header
            129, 220, 0, 0,  // Second unsupported header
        ]);

        let output = unmarshall_compound_rtcp(input).unwrap();

        assert!(output.is_empty())
    }

    #[test]
    fn rejects_malformed_compound() {
        let input = Bytes::from_static(&
        // First Packet
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1,  // Sender ssrc = 1
            0, 0, 0, 2,  // Media ssrc = 2
            1, 0, 0, 2,   // Generic NACK
            // Non-RTCP header packet
            12, 220, 0, 0,
            // Second Packet
            129, 206, 0, 2, // Payload Specific PLI Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 2 // Media SSRC = 2
        ]);

        let output = unmarshall_compound_rtcp(input);

        assert!(output.is_err())
    }
}