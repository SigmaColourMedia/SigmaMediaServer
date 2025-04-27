use bytes::Bytes;
use rtcp::header::Header;

use rtcp::Unmarshall;

use crate::media_header::{MediaHeader, RTPHeader};
use crate::stun::{get_stun_packet, ICEStunMessageType};

pub enum PacketType {
    RTP(RTPHeader),
    RTCP(Header),
    STUN(ICEStunMessageType),
    Unknown,
}

// todo Refactor and optimize this
pub fn get_packet_type(bytes: Bytes) -> PacketType {
    if let Some(ice_message) = get_stun_packet(&bytes) {
        return PacketType::STUN(ice_message);
    }

    if let Ok(media_header) = MediaHeader::unmarshall(bytes) {
        match media_header {
            MediaHeader::RTP(rtp_header) => return PacketType::RTP(rtp_header),
            MediaHeader::RTCP(rtcp_header) => return PacketType::RTCP(rtcp_header),
        }
    };

    PacketType::Unknown
}
