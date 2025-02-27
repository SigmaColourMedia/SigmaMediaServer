use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use crate::header::Header;
use crate::{Unmarshall, UnmarshallError};


#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SenderReport {
    header: Header,
    sender_info: SenderInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SenderInfo {
    pub sender_ssrc: u32,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub sender_packet_count: u32,
    pub sender_octet_count: u32,
}


impl Unmarshall for SenderReport {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes.clone())?;
        let mut reader = bytes.into_iter().skip(4).collect::<Bytes>().reader();
        let sender_ssrc = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;
        let ntp_timestamp = reader.read_u64::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;
        let rtp_timestamp = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;
        let sender_packet_count = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;
        let sender_octet_count = reader.read_u32::<BigEndian>().or(Err(UnmarshallError::UnexpectedFrame))?;

        Ok(Self {
            header,
            sender_info: SenderInfo {
                ntp_timestamp,
                sender_ssrc,
                sender_packet_count,
                sender_octet_count,
                rtp_timestamp,
            },
        })
    }
}


#[cfg(test)]
mod sender_report_unmarshall {
    use bytes::Bytes;
    use crate::header::{Header, PayloadType};
    use crate::sender_report::{SenderInfo, SenderReport};
    use crate::Unmarshall;

    #[test]
    fn unmarshall_ok_report() {
        let input = Bytes::from_static(&[
            128, 200, 0, 6, // Header
            29, 71, 245, 255, // Sender SSRC = 491255295
            235, 90, 32, 152, // NTP timestamp MS
            101, 67, 120, 0, // NTP timestamp LS, NTP(64 bits) = 16958903185723062272
            28, 55, 243, 233, // RTP timestamp = 473428969
            0, 0, 15, 25, // Sender packet count = 3865
            0, 35, 115, 177], // Sender octet count = 2323377
        );

        let expected_output = SenderReport {
            header: Header {
                padding: false,
                length: 6,
                payload_type: PayloadType::SenderReport,
                feedback_message_type: 0,
            },
            sender_info: SenderInfo {
                ntp_timestamp: 16958903185723062272,
                rtp_timestamp: 473428969,
                sender_octet_count: 2323377,
                sender_packet_count: 3865,
                sender_ssrc: 491255295,
            },
        };
        assert_eq!(SenderReport::unmarshall(input).unwrap(), expected_output);
    }
}

// 129, 202, 0, 6, // SDES
// 29, 71, 245, 255,
// 1, 16, 120, 53,
// 114, 51, 52, 49,
// 51, 81, 102, 122,
// 68, 86, 79, 84,
// 79, 56, 0, 0]