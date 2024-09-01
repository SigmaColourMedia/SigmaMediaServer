use byteorder::{ByteOrder, NetworkEndian};

use sdp::NegotiatedSession;

/**
https://datatracker.ietf.org/doc/html/rfc3550#section-5.1
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|X|  CC   |M|     PT      |       sequence number         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           timestamp                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           synchronization source (SSRC) identifier            |
+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+
|            contributing source (CSRC) identifiers             |
|                             ....                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
*/
pub fn remap_rtp_header(
    buffer: &mut [u8],
    streamer_session: &NegotiatedSession,
    viewer_session: &NegotiatedSession,
) {
    let mapped_header = get_mapped_header(
        get_rtp_header_data(buffer),
        streamer_session,
        viewer_session,
    );

    // Second byte contains for Marker & PayloadType fields.
    // Marker is the most significant bit, the rest 7-bits make for the payload number;
    let marker_bit = if mapped_header.marker_set {
        0b1000_0000
    } else {
        0
    };
    let remaped_second_byte = marker_bit ^ mapped_header.payload_type;

    // Replace second byte so that PT changes to target_payload_number
    buffer[1] = remaped_second_byte;

    // Replace SSRC bits with new ssrc value
    NetworkEndian::write_u32(&mut buffer[8..12], mapped_header.ssrc);
}

fn get_mapped_header(
    original_header: RTPHeader,
    streamer_session: &NegotiatedSession,
    viewer_session: &NegotiatedSession,
) -> RTPHeader {
    if streamer_session.audio_session.payload_number == original_header.payload_type as usize {
        RTPHeader {
            ssrc: viewer_session.audio_session.host_ssrc,
            payload_type: viewer_session.audio_session.payload_number as u8,
            marker_set: original_header.marker_set,
        }
    } else {
        RTPHeader {
            ssrc: viewer_session.video_session.host_ssrc,
            payload_type: viewer_session.video_session.payload_number as u8,
            marker_set: original_header.marker_set,
        }
    }
}

pub struct RTPHeader {
    marker_set: bool,
    pub payload_type: u8,
    ssrc: u32,
}
pub fn get_rtp_header_data(buffer: &[u8]) -> RTPHeader {
    let first_byte = buffer[1];

    let marker_set = (first_byte & 0b1000_0000) == 0b1000_0000;
    let payload_type = first_byte & 0b0111_1111;
    let ssrc = NetworkEndian::read_u32(&buffer[8..12]);

    RTPHeader {
        payload_type,
        marker_set,
        ssrc,
    }
}
