use std::io::{BufRead, Read};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use crate::{Unmarshall, UnmarshallError};
use crate::header::Header;


#[derive(Debug, PartialEq)]
pub(crate) struct TransportLayerNACK {
    pub(crate) sender_ssrc: u32,
    pub(crate) media_ssrc: u32,
    pub(crate) nacks: Vec<GenericNACK>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct GenericNACK {
    pub(crate) pid: u16,
    pub(crate) blp: u16,
}

impl Unmarshall for TransportLayerNACK {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes.clone())?;

        if header.feedback_message_type != GENERIC_NACK_FMT {
            return Err(UnmarshallError::UnexpectedFrame);
        }


        let mut reader = bytes.into_iter().skip(4).collect::<Bytes>().reader();
        let sender_ssrc = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;
        let media_ssrc = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;


        let mut nacks = vec![];
        let mut nack_buff = [0u8; 4];
        while let Ok(_) = reader.read_exact(&mut nack_buff) {
            let input = Bytes::copy_from_slice(&nack_buff);
            let nack = GenericNACK::unmarshall(input)?;
            nacks.push(nack)
        }

        Ok(TransportLayerNACK {
            nacks,
            media_ssrc,
            sender_ssrc,
        })
    }
}

impl Unmarshall for GenericNACK {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError> {
        let mut reader = bytes.reader();
        let pid = reader
            .read_u16::<BigEndian>()
            .or(Err(UnmarshallError::UnexpectedFrame))?;
        let blp = reader
            .read_u16::<BigEndian>()
            .or(Err(UnmarshallError::UnexpectedFrame))?;

        Ok(GenericNACK { blp, pid })
    }
}

#[cfg(test)]
mod tlf_nack_tests {
    use super::*;

    #[test]
    fn tfl_ok() {
        let bytes = Bytes::from_static(&
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
            1, 0, 0, 2 // Generic NACK
        ]);
        let tl_nack = TransportLayerNACK::unmarshall(bytes).unwrap();

        assert_eq!(tl_nack, TransportLayerNACK {
            sender_ssrc: 1,
            media_ssrc: 2,
            nacks: vec![GenericNACK {
                pid: 256,
                blp: 2,
            }],
        })
    }

    #[test]
    fn tfl_ok_with_multiple_nacks() {
        let bytes = Bytes::from_static(&
        [129, 205, 0, 3, // Transport Layer Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
            1, 0, 0, 2, // Generic NACK
            1, 2, 0, 3 // Generic NACK
        ]);
        let tl_nack = TransportLayerNACK::unmarshall(bytes).unwrap();

        assert_eq!(tl_nack, TransportLayerNACK {
            sender_ssrc: 1,
            media_ssrc: 2,
            nacks: vec![GenericNACK {
                pid: 256,
                blp: 2,
            }, GenericNACK {
                pid: 258,
                blp: 3,
            }],
        })
    }
}

#[cfg(test)]
mod generic_nack_tests {
    use super::*;

    #[test]
    fn generic_nack_ok() {
        let bytes = Bytes::from_static(&[
            66, 5, // PID = 16901
            0, 0]); // BLP = 0
        let generic_nack = GenericNACK::unmarshall(bytes).unwrap();

        assert_eq!(generic_nack, GenericNACK { pid: 16901, blp: 0 })
    }

    #[test]
    fn generic_nack_ok_with_blp() {
        let bytes = Bytes::from_static(&[
            66, 5, // PID = 16901
            2, 4]); // BLP = 516
        let generic_nack = GenericNACK::unmarshall(bytes).unwrap();

        assert_eq!(generic_nack, GenericNACK { pid: 16901, blp: 516 })
    }
}

static GENERIC_NACK_FMT: u8 = 1;