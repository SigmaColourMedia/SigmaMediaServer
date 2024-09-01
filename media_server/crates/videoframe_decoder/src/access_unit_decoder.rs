use std::mem;

use crate::nal::{FragmentationRole, get_nal_packet, NALPacket};
use crate::rtp::RTPPacket;

type AccessUnit = Vec<u8>;

pub struct AccessUnitDecoder {
    last_seq: Option<u16>,
    timestamp: Option<u32>,
    nal_decoder: NALDecoder,
    internal_buffer: Vec<u8>,
    _is_loopback: bool,
}
enum DecodeError {
    SequenceMismatch,
    TimestampMismatch,
    InvalidLastPacket,
}

impl AccessUnitDecoder {
    pub fn new() -> Self {
        AccessUnitDecoder {
            _is_loopback: false,
            nal_decoder: NALDecoder::new(),
            timestamp: None,
            last_seq: None,
            internal_buffer: vec![],
        }
    }
    pub fn process_packet(&mut self, packet: RTPPacket) -> Option<AccessUnit> {
        if packet.marker & self.timestamp.is_none() {
            self.last_seq = Some(packet.sequence_number);
            self._is_loopback = true;
            self.internal_buffer.push(0);
            return None;
        }

        if self._is_loopback {
            self.timestamp = Some(packet.timestamp);
            self._is_loopback = false
        }

        match self.get_nal(packet.clone()) {
            Ok(buff) => {
                self.last_seq = Some(packet.sequence_number);

                if let Some(mut nal) = buff {
                    let nal_header = nal[0];
                    let nal_unit_type = nal_header & 0b0001_1111;

                    if nal_unit_type == 7 || nal_unit_type == 8 {
                        self.internal_buffer.push(0);
                    }

                    self.internal_buffer.extend_from_slice(&[0, 0, 1]);
                    self.internal_buffer.append(&mut nal)
                }

                let is_last_packet = packet.marker;
                if is_last_packet {
                    println!("producing packet {}", self.last_seq.unwrap());
                    Some(mem::replace(&mut self.internal_buffer, vec![]))
                } else {
                    None
                }
            }
            Err(_) => {
                self.internal_buffer.clear();
                self.nal_decoder = NALDecoder::new();
                self.last_seq = None;
                self.timestamp = None;
                None
            }
        }
    }

    fn get_nal(&mut self, packet: RTPPacket) -> Result<Option<Vec<u8>>, DecodeError> {
        let is_last_packet_in_access_unit = packet.marker;
        let is_next_in_seq = self
            .last_seq
            .map(|seq| {
                if seq == u16::MAX {
                    return packet.sequence_number == 0;
                }
                return packet.sequence_number == seq + 1;
            })
            .and_then(|val| val.then_some(()))
            .is_some();

        let is_matching_timestamp = self
            .timestamp
            .map(|seq| seq.eq(&packet.timestamp))
            .and_then(|val| val.then_some(()))
            .is_some();

        if !is_next_in_seq {
            return Err(DecodeError::SequenceMismatch);
        }
        if !is_matching_timestamp {
            return Err(DecodeError::TimestampMismatch);
        }
        let nal_unit = self.nal_decoder.decode_nal_unit(packet);

        if is_last_packet_in_access_unit && nal_unit.is_none() {
            return Err(DecodeError::InvalidLastPacket);
        }
        return Ok(nal_unit);
    }
}

pub struct NALDecoder {
    fragmentation_buffer: Vec<u8>,
}

impl NALDecoder {
    pub fn new() -> Self {
        NALDecoder {
            fragmentation_buffer: vec![],
        }
    }

    pub fn decode_nal_unit(&mut self, rtp_packet: RTPPacket) -> Option<Vec<u8>> {
        let nal_packet = get_nal_packet(rtp_packet.payload.as_slice())?;

        match nal_packet {
            NALPacket::NALUnit(mut unit_packet) => Some(unit_packet.unit),
            NALPacket::FragmentationUnit(mut frag) => {
                match frag.fragmentation_header.fragmentation_role {
                    FragmentationRole::Start => {
                        self.fragmentation_buffer.clear();
                        let unit_header_prefix: u8 = u8::from(frag.unit_header) & 0b1110_0000;
                        let unit_payload_type = frag.fragmentation_header.nal_payload_type;
                        let header = unit_header_prefix ^ unit_payload_type;
                        self.fragmentation_buffer.push(header); // Append NAL Unit header
                        self.fragmentation_buffer.append(&mut frag.unit); // Append payload
                        None
                    }
                    FragmentationRole::Continue => {
                        self.fragmentation_buffer.append(&mut frag.unit);
                        None
                    }
                    FragmentationRole::End => {
                        self.fragmentation_buffer.append(&mut frag.unit);
                        Some(mem::replace(&mut self.fragmentation_buffer, vec![]))
                    }
                }
            }
        }
    }
}
