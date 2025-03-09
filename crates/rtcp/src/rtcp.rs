use bytes::Bytes;
use crate::payload_specific_feedback::PayloadSpecificFeedback;
use crate::transport_layer_feedback::TransportLayerNACK;
use crate::{Marshall, MarshallError, Unmarshall, UnmarshallError};
use crate::header::{Header, PayloadType};
use crate::sdes::SourceDescriptor;
use crate::sender_report::SenderReport;

#[derive(Debug, PartialEq)]
pub enum RtcpPacket {
    TransportLayerFeedbackMessage(TransportLayerNACK),
    PayloadSpecificFeedbackMessage(PayloadSpecificFeedback),
    SenderReport(SenderReport),
    SourceDescriptor(SourceDescriptor),
}

impl Marshall for RtcpPacket {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        match self {
            RtcpPacket::TransportLayerFeedbackMessage(tlp_fb) => tlp_fb.marshall(),
            RtcpPacket::PayloadSpecificFeedbackMessage(ps_fb) => ps_fb.marshall(),
            _ => {
                panic!("Cannot marshall unsupported packet")
            }
        }
    }
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
            PayloadType::SenderReport => Ok(RtcpPacket::SenderReport(SenderReport::unmarshall(bytes)?)),
            PayloadType::SDES => Ok(RtcpPacket::SourceDescriptor(SourceDescriptor::unmarshall(bytes)?)),
            _ => Err(UnmarshallError::UnexpectedFrame)
        }
    }
}


#[cfg(test)]
mod rtcp_marshall {
    use crate::payload_specific_feedback::PictureLossIndication;
    use crate::transport_layer_feedback::GenericNACK;
    use super::*;

    #[test]
    fn tl_nack_ok() {
        let input = RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK {
            header: Header {
                payload_type: PayloadType::TransportLayerFeedbackMessage,
                feedback_message_type: 1,
                length: 3,
                padding: false,
            },
            sender_ssrc: 1,
            media_ssrc: 2,
            nacks: vec![GenericNACK {
                pid: 256,
                blp: 2,
            }],
        });

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1,  // Sender ssrc = 1
            0, 0, 0, 2,  // Media ssrc = 2
            1, 0, 0, 2   // Generic NACK
        ]))
    }

    #[test]
    fn ps_fb_ok() {
        let input = RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication {
            media_ssrc: 2,
            sender_ssrc: 1,
            header: Header {
                padding: false,
                length: 2,
                feedback_message_type: 1,
                payload_type: PayloadType::PayloadSpecificFeedbackMessage,
            },
        }));

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&
        [129, 206, 0, 2, // Feedback Specific Layer Feedback Header
            0, 0, 0, 1,  // Sender ssrc = 1
            0, 0, 0, 2,  // Media ssrc = 2
        ]))
    }
}

#[cfg(test)]
mod rtcp_unmarshall {
    use crate::payload_specific_feedback::PictureLossIndication;
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
            header: Header {
                payload_type: PayloadType::TransportLayerFeedbackMessage,
                feedback_message_type: 1,
                length: 3,
                padding: false,
            },
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

        assert_eq!(rtcp_packet, RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication {
            media_ssrc: 2,
            sender_ssrc: 1,
            header: Header {
                padding: false,
                length: 2,
                feedback_message_type: 1,
                payload_type: PayloadType::PayloadSpecificFeedbackMessage,
            },
        })))
    }
}
