use byteorder::{WriteBytesExt};
use bytes::{BufMut, Bytes, BytesMut};
use crate::header::{Header, PayloadType};
use crate::{Marshall, MarshallError};

#[derive(Debug, Clone, PartialEq)]
pub struct ReceiverReport {
    header: Header,
    sender_ssrc: u32,
    reports: Vec<ReportBlock>,
}

impl ReceiverReport {
    pub fn new(sender_ssrc: u32, reports: Vec<ReportBlock>) -> Self {
        let header = Header {
            length: (1 + reports.len() * 6) as u16, // Sender SSRC 32-bit word + N-report_blocks * 6 32-bit words
            payload_type: PayloadType::ReceiverReport,
            padding: false,
            feedback_message_type: reports.len() as u8,
        };

        Self { reports, sender_ssrc, header }
    }
}

impl Marshall for ReceiverReport {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        bytes.put(self.header.marshall()?);
        bytes.put_u32(self.sender_ssrc);
        for report in self.reports {
            bytes.put(report.marshall()?);
        };
        Ok(bytes.freeze())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_packets_lost: u32,
    pub ext_highest_sequence: u32,
    pub jitter: u32,
    pub lsr: u32,
    pub dlsr: u32,
}

impl Marshall for ReportBlock {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        bytes.put_u32(self.ssrc);
        bytes.put_u8(self.fraction_lost);
        let cumulative_packet_lost_frame = self.cumulative_packets_lost.to_be_bytes()[1..].to_vec();
        bytes.put(Bytes::from(cumulative_packet_lost_frame));
        bytes.put_u32(self.ext_highest_sequence);
        bytes.put_u32(self.jitter);
        bytes.put_u32(self.lsr);
        bytes.put_u32(self.dlsr);
        Ok(bytes.freeze())
    }
}

#[cfg(test)]
mod receiver_report_new_constructor {
    use crate::header::{Header, PayloadType};
    use crate::receiver_report::{ReceiverReport, ReportBlock};

    #[test]
    fn one_block_receiver_report() {
        let actual_output = ReceiverReport::new(1, vec![ReportBlock {
            lsr: 0,
            ssrc: 2,
            dlsr: 0,
            fraction_lost: 10,
            jitter: 0,
            cumulative_packets_lost: 2,
            ext_highest_sequence: 15,
        }]);

        let expected_output = ReceiverReport {
            header: Header {
                padding: false,
                length: 7,
                payload_type: PayloadType::ReceiverReport,
                feedback_message_type: 1,
            },
            sender_ssrc: 1,
            reports: vec![ReportBlock {
                lsr: 0,
                ssrc: 2,
                dlsr: 0,
                fraction_lost: 10,
                jitter: 0,
                cumulative_packets_lost: 2,
                ext_highest_sequence: 15,
            }],
        };

        assert_eq!(actual_output, expected_output)
    }
}

#[cfg(test)]
mod receiver_report_marshall {
    use bytes::Bytes;
    use crate::header::{Header, PayloadType};
    use crate::Marshall;
    use crate::receiver_report::{ReceiverReport, ReportBlock};

    #[test]
    fn marshall_one_block() {
        let input = ReceiverReport {
            sender_ssrc: 1,
            header: Header {
                padding: false,
                length: 7,
                payload_type: PayloadType::ReceiverReport,
                feedback_message_type: 1,
            },
            reports: vec![ReportBlock {
                ssrc: 123213414,
                fraction_lost: 20,
                cumulative_packets_lost: 2120,
                ext_highest_sequence: 32131,
                jitter: 1200,
                lsr: 230232,
                dlsr: 200,
            }],
        };

        let expected_output = Bytes::from_static(&[
            129, 201, 0, 7, // Header, report blocks = 1, length = 7
            0, 0, 0, 1, // Sender SSRC = 1
            7, 88, 22, 102, // SSRC = 123213414
            20, 0, 8, 72, // Fraction Lost = 20, Packets Lost = 2120
            0, 0, 125, 131, // Extended Highest Sequence =  32131
            0, 0, 4, 176, // Jitter = 1200
            0, 3, 131, 88, // LSR = 230232,
            0, 0, 0, 200 // DLSR = 200
        ]);


        assert_eq!(input.marshall().unwrap(), expected_output);
    }
}


#[cfg(test)]
mod report_block_marshall {
    use bytes::Bytes;
    use crate::Marshall;
    use crate::receiver_report::ReportBlock;

    #[test]
    fn marshall_ok() {
        let input = ReportBlock {
            ssrc: 123213414,
            fraction_lost: 20,
            cumulative_packets_lost: 2120,
            ext_highest_sequence: 32131,
            jitter: 1200,
            lsr: 230232,
            dlsr: 200,
        };
        let output = input.marshall().unwrap();

        let expected_output = Bytes::from_static(&[
            7, 88, 22, 102, // SSRC = 123213414
            20, 0, 8, 72, // Fraction Lost = 20, Packets Lost = 2120
            0, 0, 125, 131, // Extended Highest Sequence =  32131
            0, 0, 4, 176, // Jitter = 1200
            0, 3, 131, 88, // LSR = 230232,
            0, 0, 0, 200 // DLSR = 200
        ]);

        assert_eq!(output, expected_output);
    }
}