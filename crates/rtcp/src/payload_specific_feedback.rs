use bytes::Bytes;
use crate::{Unmarshall, UnmarshallError};
use crate::header::Header;

#[derive(Debug, PartialEq)]
pub(crate) enum PayloadSpecificFeedback {
    PictureLossIndication
}

impl Unmarshall for PayloadSpecificFeedback {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes)?;
        if header.length != 2 {
            return Err(UnmarshallError::UnexpectedFrame);
        }

        match &header.feedback_message_type {
            1 => Ok(PayloadSpecificFeedback::PictureLossIndication),
            _ => return Err(UnmarshallError::UnexpectedFrame)
        }
    }
}


#[cfg(test)]
mod pls_fb_tests {
    use super::*;


    #[test]
    fn pls_fb_ok() {
        let bytes = Bytes::from_static(&
        [129, 206, 0, 2, // Payload Specific Feedback Header
            0, 0, 0, 1, // Sender ssrc = 1
            0, 0, 0, 2, // Media ssrc = 2
        ]);
        let pls_fb = PayloadSpecificFeedback::unmarshall(bytes).unwrap();

        assert_eq!(pls_fb, PayloadSpecificFeedback::PictureLossIndication)
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