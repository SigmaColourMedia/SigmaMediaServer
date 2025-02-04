use std::io::{BufRead, Read};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::{Marshall, MarshallError, Unmarshall, UnmarshallError};
use crate::header::{Header, PayloadType};


#[derive(Debug, PartialEq)]
pub struct TransportLayerNACK {
    pub header: Header,
    pub sender_ssrc: u32,
    pub media_ssrc: u32,
    pub nacks: Vec<GenericNACK>,
}

#[derive(Debug, PartialEq)]
pub struct GenericNACK {
    pub pid: u16,
    pub blp: u16,
}

impl TransportLayerNACK {
    pub fn new(nacks: Vec<GenericNACK>, sender_ssrc: u32, media_ssrc: u32) -> Self {
        if nacks.len() < 1 {
            panic!("Packet must contain at least one Generic NACK")
        };

        let header = Header {
            feedback_message_type: 1,
            payload_type: PayloadType::TransportLayerFeedbackMessage,
            padding: false,
            length: nacks.len() as u16 + 2,
        };

        Self {
            header,
            nacks,
            media_ssrc,
            sender_ssrc,
        }
    }
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
            header,
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

impl Marshall for TransportLayerNACK {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        bytes.put(self.header.marshall()?);
        bytes.put_u32(self.sender_ssrc);
        bytes.put_u32(self.media_ssrc);

        for nack in self.nacks {
            bytes.put(nack.marshall()?)
        }

        Ok(bytes.freeze())
    }
}

impl Marshall for GenericNACK {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        bytes.put_u16(self.pid);
        bytes.put_u16(self.blp);
        Ok(bytes.freeze())
    }
}

#[cfg(test)]
mod marshall_tl_fb_nack {
    use crate::header::PayloadType;
    use super::*;

    #[test]
    fn marshall_ok() {
        let input = TransportLayerNACK {
            header: Header {
                padding: false,
                length: 3,
                payload_type: PayloadType::TransportLayerFeedbackMessage,
                feedback_message_type: 1,
            },
            sender_ssrc: 1,
            media_ssrc: 2,
            nacks: vec![GenericNACK {
                pid: 150,
                blp: 0,
            }],
        };

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            129, 205, 0, 3, // Transport Layer Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 2, // Media SSRC = 2
            0, 150, 0, 0 // Generic NACK, PID=150 BLP=0
        ]))
    }

    #[test]
    fn marshall_ok_multiple_nacks() {
        let input = TransportLayerNACK {
            header: Header {
                padding: false,
                length: 3,
                payload_type: PayloadType::TransportLayerFeedbackMessage,
                feedback_message_type: 1,
            },
            sender_ssrc: 1,
            media_ssrc: 2,
            nacks: vec![GenericNACK {
                pid: 150,
                blp: 0,
            }, GenericNACK {
                pid: 256,
                blp: 2,
            }],
        };

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            129, 205, 0, 3, // Transport Layer Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 2, // Media SSRC = 2
            0, 150, 0, 0, // Generic NACK, PID=150 BLP=0
            1, 0, 0, 2 // Generic NACK, PID=256 BLP = 2
        ]))
    }
}

#[cfg(test)]
mod marshall_generic_nack_tests {
    use super::*;
    use crate::Marshall;
    use crate::transport_layer_feedback::GenericNACK;

    #[test]
    fn marshall_ok() {
        let input = GenericNACK {
            pid: 150,
            blp: 0,
        };

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            0, 150, // PID = 150
            0, 0])) // blp = 0
    }

    #[test]
    fn marshall_ok_with_blp() {
        let input = GenericNACK {
            pid: 16901,
            blp: 150,
        };

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            66, 5, // PID = 16901
            0, 150])) // BLP = 150
    }
}

#[cfg(test)]
mod unmarshall_tlf_nack_tests {
    use crate::header::PayloadType;
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
            }, GenericNACK {
                pid: 258,
                blp: 3,
            }],
        })
    }
}

#[cfg(test)]
mod unmarshall_generic_nack_tests {
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