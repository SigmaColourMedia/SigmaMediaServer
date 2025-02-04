use bytes::Bytes;
use rtcp::payload_specific_feedback::{PayloadSpecificFeedback, PictureLossIndication};
use rtcp::rtcp::RtcpPacket;
use rtcp::transport_layer_feedback::{GenericNACK, TransportLayerNACK};
use rtcp::unmarshall_compound_rtcp;

#[test]
fn compound_with_rr_and_nack_ok() {
    let bytes = Bytes::from_static(&[129, 201, 0, 7, 0, 0, 0, 1, 90, 210, 246, 189, 0, 0, 0, 128, 0, 0, 70, 42, 0, 0, 5, 155, 0, 0, 0, 0, 0, 0, 0, 0, 129, 205, 0, 14, 0, 0, 0, 1, 90, 210, 246, 189, 66, 5, 0, 0, 66, 26, 0, 32, 66, 54, 0, 64, 67, 49, 0, 32, 67, 69, 16, 0, 67, 92, 0, 32, 68, 92, 24, 0, 68, 113, 4, 8, 68, 132, 0, 0, 69, 127, 0, 0, 69, 148, 4, 131, 69, 168, 1, 0]);

    let output = unmarshall_compound_rtcp(bytes).unwrap();
    let nack = RtcpPacket::TransportLayerFeedbackMessage(TransportLayerNACK::new(vec![
        GenericNACK {
            pid: 16901,
            blp: 0,
        },
        GenericNACK {
            pid: 16922,
            blp: 32,
        }, GenericNACK {
            pid: 16950,
            blp: 64,
        },
        GenericNACK {
            pid: 17201,
            blp: 32,
        }, GenericNACK {
            pid: 17221,
            blp: 4096,
        }, GenericNACK {
            pid: 17244,
            blp: 32,
        }, GenericNACK {
            pid: 17500,
            blp: 6144,
        }, GenericNACK {
            pid: 17521,
            blp: 1032,
        }, GenericNACK {
            pid: 17540,
            blp: 0,
        }, GenericNACK {
            pid: 17791,
            blp: 0,
        }, GenericNACK {
            pid: 17812,
            blp: 1155,
        }, GenericNACK {
            pid: 17832,
            blp: 256,
        }], 1, 1523775165));

    assert_eq!(output, vec![nack])
}

#[test]
fn compound_with_rr_and_pli() {
    let input = Bytes::from_static(&[129, 201, 0, 7, 210, 182, 140, 126, 178, 47, 77, 245, 0, 0, 0, 0, 0, 0, 6, 5, 0, 0, 1, 117, 0, 0, 0, 0, 0, 0, 0, 0, 129, 206, 0, 2, 210, 182, 140, 126, 178, 47, 77, 245]);
    let output = unmarshall_compound_rtcp(input).unwrap();
    let expected_output = vec![RtcpPacket::PayloadSpecificFeedbackMessage(PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication::new(3535178878, 2989444597)))];

    assert_eq!(output, expected_output)
}