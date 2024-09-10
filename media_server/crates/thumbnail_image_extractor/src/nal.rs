use std::fmt::{Display, Formatter, Pointer};

/**
https://datatracker.ietf.org/doc/html/rfc6184#section-5.3
 +---------------+
 |0|1|2|3|4|5|6|7|
 +-+-+-+-+-+-+-+-+
 |F|NRI|  Type   |
 +---------------+
*/

#[derive(Debug)]
pub struct NALUnitHeader {
    nri: u8,
    _inner: u8,
    payload_type: PayloadType,
}
#[derive(Debug)]
pub enum PayloadType {
    NALUnit,
    FU_A,
    FU_B,
    STAP_A,
    STAP_B,
    MTAP16,
    MTAP24,
    Reserved,
}
#[derive(Debug)]
pub enum ParseError {
    MalformedPacket,
}

impl TryFrom<u8> for PayloadType {
    type Error = ParseError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PayloadType::Reserved),
            1..=23 => Ok(PayloadType::NALUnit),
            24 => Ok(PayloadType::STAP_A),
            25 => Ok(PayloadType::STAP_B),
            26 => Ok(PayloadType::MTAP16),
            27 => Ok(PayloadType::MTAP24),
            28 => Ok(PayloadType::FU_A),
            29 => Ok(PayloadType::FU_B),
            30..=31 => Ok(PayloadType::Reserved),
            _ => Err(Self::Error::MalformedPacket),
        }
    }
}

impl TryFrom<u8> for NALUnitHeader {
    type Error = ParseError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let forbidden_zero_bit = (value & 0b1000_0000) >> 7;
        if forbidden_zero_bit == 1 {
            return Err(Self::Error::MalformedPacket);
        }

        let nri = (value & 0b0110_0000) >> 5;

        let payload_type_number = value & 0b0001_1111;

        let payload_type = PayloadType::try_from(payload_type_number)?;

        Ok(Self {
            nri,
            payload_type,
            _inner: value,
        })
    }
}

impl From<NALUnitHeader> for u8 {
    fn from(value: NALUnitHeader) -> Self {
        value._inner
    }
}

/**
https://datatracker.ietf.org/doc/html/rfc6184#section-5.8
+---------------+
|0|1|2|3|4|5|6|7|
+-+-+-+-+-+-+-+-+
|S|E|R|  Type   |
+---------------+
*/
#[derive(Debug)]
pub(crate) struct NALFragmentationHeader {
    pub(crate) fragmentation_role: FragmentationRole,
    pub(crate) nal_payload_type: u8,
}
#[derive(Debug)]
pub(crate) enum FragmentationRole {
    Start,
    Continue,
    End,
}

impl Display for FragmentationRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FragmentationRole::Start => write!(f, "Start"),
            FragmentationRole::Continue => write!(f, "Continue"),
            FragmentationRole::End => write!(f, "End"),
        }
    }
}
impl TryFrom<u8> for NALFragmentationHeader {
    type Error = ParseError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let start_bit = (value & 0b1000_0000) >> 7;
        let end_bit = (value & 0b0100_0000) >> 6;

        let mut fragmentation_role = None;

        if start_bit == end_bit {
            // Two bits cannot be set at once
            if start_bit == 1 {
                return Err(Self::Error::MalformedPacket);
            }
            fragmentation_role = Some(FragmentationRole::Continue)
        } else {
            fragmentation_role = if start_bit == 1 {
                Some(FragmentationRole::Start)
            } else {
                Some(FragmentationRole::End)
            }
        }

        let payload_type = value & 0b0001_1111;

        Ok(Self {
            fragmentation_role: fragmentation_role.expect("Fragmentation role should be defined"),
            nal_payload_type: payload_type,
        })
    }
}

#[derive(Debug)]
pub(crate) enum NALPacket {
    NALUnit(NALUnit),
    FragmentationUnit(FragmentationUnit),
}

impl Display for NALPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NALPacket::NALUnit(unit) => {
                write!(f, "Single NAL Unit of {} length", unit.unit.len())
            }
            NALPacket::FragmentationUnit(frag) => {
                write!(
                    f,
                    "Fragmentation Unit of length={}, PT={}, role={}",
                    frag.unit.len(),
                    frag.fragmentation_header.nal_payload_type,
                    frag.fragmentation_header.fragmentation_role
                )
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct FragmentationUnit {
    pub(crate) unit_header: NALUnitHeader,
    pub(crate) fragmentation_header: NALFragmentationHeader,
    pub(crate) unit: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct NALUnit {
    pub(crate) unit: Vec<u8>,
}

pub fn get_nal_packet(input: &[u8]) -> Option<NALPacket> {
    let nal_unit_header = NALUnitHeader::try_from(input[0]).ok()?;

    match &nal_unit_header.payload_type {
        PayloadType::NALUnit => {
            let mut buffer = Vec::from(&input[0..]);
            Some(NALPacket::NALUnit(NALUnit { unit: buffer }))
        }
        PayloadType::FU_A => {
            let fragmentation_header = NALFragmentationHeader::try_from(input[1]).ok()?;
            let mut buffer = Vec::from(&input[2..]);

            Some(NALPacket::FragmentationUnit(FragmentationUnit {
                unit_header: nal_unit_header,
                fragmentation_header,
                unit: buffer,
            }))
        }
        _ => None,
    }
}
