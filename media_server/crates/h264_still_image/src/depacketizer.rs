use std::mem;

use crate::nal::{FragmentationRole, get_nal_packet, NALPacket};
use crate::rtp::RTPPacket;

pub struct NALDecoder {
    fragmentation_buffer: Vec<u8>,
    last_seq: u16,
}

impl NALDecoder {
    pub fn new() -> Self {
        NALDecoder {
            last_seq: 0,
            fragmentation_buffer: vec![],
        }
    }

    pub fn decode_nal_unit(&mut self, rtp_packet: RTPPacket) -> Option<Vec<u8>> {
        let nal_packet = get_nal_packet(rtp_packet.payload.as_slice())?;

        match nal_packet {
            NALPacket::NALUnit(mut unit_packet) => {
                let mut buffer = vec![0u8, 0, 1];
                buffer.append(&mut unit_packet.unit);
                Some(buffer)
            }
            NALPacket::FragmentationUnit(mut frag) => {
                match frag.fragmentation_header.fragmentation_role {
                    FragmentationRole::Start => {
                        self.fragmentation_buffer.clear();
                        self.fragmentation_buffer.extend_from_slice(&[0, 0, 1]); // Add byte-stream prefix
                        self.fragmentation_buffer.push(frag.unit_header.into()); // Append NAL Unit header
                        self.fragmentation_buffer.append(&mut frag.unit); // Append payload
                        self.last_seq = rtp_packet.sequence_number;
                        None
                    }
                    FragmentationRole::Continue => match rtp_packet.sequence_number {
                        0 => {
                            let is_wrap_around = self.last_seq == u16::MAX;

                            if !is_wrap_around {
                                return None;
                            }
                            self.last_seq = rtp_packet.sequence_number;
                            self.fragmentation_buffer.append(&mut frag.unit);
                            None
                        }
                        seq_number => {
                            let is_next_packet = seq_number == self.last_seq + 1;
                            if !is_next_packet {
                                return None;
                            }
                            self.last_seq = seq_number;
                            self.fragmentation_buffer.append(&mut frag.unit);

                            None
                        }
                    },
                    FragmentationRole::End => match rtp_packet.sequence_number {
                        0 => {
                            let is_wrap_around = self.last_seq == u16::MAX;

                            if !is_wrap_around {
                                return None;
                            }
                            self.last_seq = rtp_packet.sequence_number;
                            self.fragmentation_buffer.append(&mut frag.unit);
                            Some(mem::replace(&mut self.fragmentation_buffer, vec![]))
                        }
                        seq_number => {
                            let is_next_packet = seq_number == self.last_seq + 1;
                            if !is_next_packet {
                                return None;
                            }
                            self.last_seq = seq_number;
                            self.fragmentation_buffer.append(&mut frag.unit);

                            Some(mem::replace(&mut self.fragmentation_buffer, vec![]))
                        }
                    },
                }
            }
        }
    }
}
