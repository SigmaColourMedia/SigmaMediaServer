use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::{Marshall, MarshallError, Unmarshall, UnmarshallError};
use crate::header::{Header, PayloadType};

#[derive(Debug, PartialEq)]
pub enum PayloadSpecificFeedback {
    PictureLossIndication(PictureLossIndication)
}

impl PictureLossIndication {
    pub fn new(sender_ssrc: u32, media_ssrc: u32) -> Self {
        let header = Header {
            length: 2,
            padding: false,
            feedback_message_type: 1,
            payload_type: PayloadType::PayloadSpecificFeedbackMessage,
        };

        Self {
            sender_ssrc,
            media_ssrc,
            header,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct PictureLossIndication {
    pub(crate) sender_ssrc: u32,
    pub(crate) media_ssrc: u32,
    pub(crate) header: Header,
}

impl Unmarshall for PayloadSpecificFeedback {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes.clone())?;
        match &header.feedback_message_type {
            1 => Ok(PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication::unmarshall(bytes)?)),
            _ => return Err(UnmarshallError::UnexpectedFrame)
        }
    }
}


impl Unmarshall for PictureLossIndication {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes.clone())?;
        if header.length != PLI_LENGTH {
            return Err(UnmarshallError::UnexpectedFrame);
        }

        let mut reader = bytes.into_iter().skip(4).collect::<Bytes>().reader();
        let sender_ssrc = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::InvalidLength))?;
        let media_ssrc = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::InvalidLength))?;

        Ok(PictureLossIndication {
            media_ssrc,
            sender_ssrc,
            header,
        })
    }
}

impl Marshall for PayloadSpecificFeedback {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        match self {
            PayloadSpecificFeedback::PictureLossIndication(pli) => pli.marshall()
        }
    }
}

impl Marshall for PictureLossIndication {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        let header = Header::marshall(self.header)?;
        bytes.put(header);
        bytes.put_u32(self.sender_ssrc);
        bytes.put_u32(self.media_ssrc);

        Ok(bytes.freeze())
    }
}

#[cfg(test)]
mod marshall_tests {
    use bytes::Bytes;
    use crate::header::{Header, PayloadType};
    use crate::Marshall;
    use crate::payload_specific_feedback::{PayloadSpecificFeedback, PictureLossIndication};

    #[test]
    fn marshall_ps_pli_ok() {
        let input = PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication {
            header: Header {
                padding: false,
                length: 2,
                payload_type: PayloadType::PayloadSpecificFeedbackMessage,
                feedback_message_type: 1,
            },
            sender_ssrc: 1,
            media_ssrc: 2,
        });

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            129, 206, 0, 2, // Payload Specific Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 02 // Media SSRC = 2
        ]))
    }

    #[test]
    fn marshall_pli_ok() {
        let input = PictureLossIndication {
            header: Header {
                padding: false,
                length: 2,
                payload_type: PayloadType::PayloadSpecificFeedbackMessage,
                feedback_message_type: 1,
            },
            sender_ssrc: 1,
            media_ssrc: 2,
        };

        let output = input.marshall().unwrap();

        assert_eq!(output, Bytes::from_static(&[
            129, 206, 0, 2, // Payload Specific Header
            0, 0, 0, 1, // Sender SSRC = 1
            0, 0, 0, 02 // Media SSRC = 2
        ]))
    }
}

#[cfg(test)]
mod unmarshall_tests {
    use crate::header::PayloadType;
    use super::*;


    #[test]
    fn pls_fb_ok() {
        let bytes = Bytes::from_static(&
        [129, 206, 0, 2, // Payload Specific Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
        ]);
        let pls_fb = PayloadSpecificFeedback::unmarshall(bytes).unwrap();

        assert_eq!(pls_fb, PayloadSpecificFeedback::PictureLossIndication(PictureLossIndication {
            media_ssrc: 2,
            sender_ssrc: 1,
            header: Header {
                payload_type: PayloadType::PayloadSpecificFeedbackMessage,
                length: 2,
                feedback_message_type: 1,
                padding: false,
            },
        }))
    }

    #[test]
    fn pls_fb_invalid_length() {
        let bytes = Bytes::from_static(&
        [129, 206, 0, 3, // Payload Specific Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
        ]);
        let pls_fb = PayloadSpecificFeedback::unmarshall(bytes);

        assert_eq!(pls_fb.unwrap_err(), UnmarshallError::UnexpectedFrame)
    }
}

static PLI_LENGTH: u16 = 2;