use bytes::Bytes;

use rtcp::Unmarshall;

use crate::media_header::MediaHeader;
use crate::stun::get_stun_packet;

pub enum PacketType {
    RTP,
    RTCP,
    STUN,
    Unknown,
}

// todo Refactor and optimize this
pub fn get_packet_type(bytes: Bytes) -> PacketType {
    if get_stun_packet(&bytes).is_some() {
        return PacketType::STUN;
    }

    if let Ok(media_header) = MediaHeader::unmarshall(bytes) {
        match media_header {
            MediaHeader::RTP(_) => return PacketType::RTP,
            MediaHeader::RTCP(_) => return PacketType::RTCP,
        }
    };

    PacketType::Unknown
}
