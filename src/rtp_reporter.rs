use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
struct RTPReporter {
    max_seq: u16,
    cycles: u32,
    base_seq: u32,
    bad_seq: u32,
    received: u32,
    expected_prior: u32,
    received_prior: u32,
    lost_packets: HashSet<u16>,
}

#[derive(Debug, Clone, PartialEq)]
enum UpdateSequenceError {
    InvalidSequence
}

impl RTPReporter {
    fn new(seq: u16) -> Self {
        Self {
            cycles: 0,
            max_seq: seq,
            base_seq: seq as u32,
            bad_seq: RTP_SEQ_MOD + 1,
            received: 1,
            received_prior: 0,
            expected_prior: 0,
            lost_packets: HashSet::new(),
        }
    }


    fn update_seq(&mut self, seq: u16) -> Result<(), UpdateSequenceError> {
        let u_delta = seq.wrapping_sub(self.max_seq);

        // in order, with permissible gap
        if u_delta < MAX_DROPOUT {


            // Sequence number wrapped
            if seq < self.max_seq {
                // Add any missing packets in previous cycle
                let packets_missed_in_previous_cycle = u16::MAX - self.max_seq;
                for packet in 0..packets_missed_in_previous_cycle {
                    self.lost_packets.insert(packet + self.max_seq + 1);
                }
                // Add any missing packets in new cycle
                let packets_missed_in_new_cycle = seq;
                for packet in 0..packets_missed_in_new_cycle {
                    self.lost_packets.insert(packet);
                }
                //  count another cycle.
                self.cycles += RTP_SEQ_MOD;
            }
            // In cycle, jumped few packets forwards
            else {
                // Add any missing packets in current cycle
                let packets_missed_in_cycle = seq - self.max_seq - 1;
                for packet in 0..packets_missed_in_cycle {
                    self.lost_packets.insert(packet + self.max_seq + 1);
                }
            }


            self.max_seq = seq;

            /* the sequence number made a very large jump */
        } else if u_delta as u32 <= RTP_SEQ_MOD - MAX_MISORDER {
            /*
             * Two sequential packets -- assume that the other side
             * restarted without telling us so just re-sync
             * (i.e., pretend this was the first packet).
             */
            if seq as u32 == self.bad_seq {
                let _ = std::mem::replace(self, RTPReporter::new(seq));
                // Bad packet, await for next sequential packet
            } else {
                self.bad_seq = (seq as u32 + 1) & (RTP_SEQ_MOD - 1);
                return Err(UpdateSequenceError::InvalidSequence);
            }
            /* duplicate or reordered packet */
        } else {
            // Evict lost packet
            self.lost_packets.remove(&seq);
        }
        self.received += 1;
        Ok(())
    }

    fn lost_packets(&self) -> u32 {
        let extended_max = self.cycles + self.max_seq as u32;
        let expected = extended_max - self.base_seq + 1;
        expected - self.received
    }

    fn fraction_lost(&self) -> u8 {
        let extended_max = self.cycles + self.max_seq as u32;
        let expected = extended_max - self.base_seq + 1;
        let expected_interval = expected - self.expected_prior;
        let received_interval = self.received - self.received_prior;
        let lost_interval = expected_interval.wrapping_sub(received_interval);
        if expected_interval == 0 || lost_interval <= 0 {
            return 0;
        }
        return ((lost_interval << 8) as u32 / expected_interval) as u8;
    }

    fn generate_receiver_report(&mut self) {}
}


static MAX_DROPOUT: u16 = 3000;
static MAX_MISORDER: u32 = 100;
static RTP_SEQ_MOD: u32 = 1 << 16;

#[cfg(test)]
mod fraction_lost {
    use std::collections::HashSet;
    use crate::rtp_reporter::{RTP_SEQ_MOD, RTPReporter};

    #[test]
    fn no_packets_lost() {
        let reporter = RTPReporter::new(1);

        let lost = reporter.fraction_lost();

        assert_eq!(lost, 0)
    }

    #[test]
    fn half_packets_lost_since_last_report() {
        let mut reporter = RTPReporter {
            lost_packets: HashSet::new(),
            received_prior: 4,
            received: 4,
            expected_prior: 4,
            max_seq: 4,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            base_seq: 1,
        };
        reporter.update_seq(6).unwrap();


        let lost = reporter.fraction_lost();
        let percentage = lost as f32 / 256.0;
        assert_eq!(percentage, 0.5)
    }

    #[test]
    fn quarter_packets_lost_since_last_report() {
        let mut reporter = RTPReporter {
            lost_packets: HashSet::new(),
            received_prior: 4,
            received: 4,
            expected_prior: 4,
            max_seq: 4,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            base_seq: 1,
        };
        reporter.update_seq(5).unwrap();
        reporter.update_seq(6).unwrap();
        reporter.update_seq(8).unwrap();


        let lost = reporter.fraction_lost();
        let percentage = lost as f32 / 256.0;
        assert_eq!(percentage, 0.25)
    }
}

#[cfg(test)]
mod update_seq {
    use std::collections::HashSet;
    use crate::rtp_reporter::{MAX_DROPOUT, RTP_SEQ_MOD, RTPReporter, UpdateSequenceError};

    #[test]
    fn packet_comes_in_order() {
        let base_seq = 2;
        let mut reporter = RTPReporter::new(base_seq);
        let next_seq = 3;
        let result = reporter.update_seq(next_seq).unwrap();

        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, next_seq);
        assert_eq!(reporter.received, 2);
    }

    #[test]
    fn packet_skips_3_seq() {
        let base_seq = 2;
        let mut reporter = RTPReporter::new(base_seq);
        let next_seq = 6;
        let result = reporter.update_seq(next_seq).unwrap();

        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, next_seq);
        assert_eq!(reporter.received, 2);
    }

    #[test]
    fn reordered_packet_comes_in() {
        let base_seq = 2;
        let mut reporter = RTPReporter::new(base_seq);
        reporter.update_seq(4).unwrap();
        reporter.update_seq(3).unwrap();

        assert_eq!(reporter.received, 3);
        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, 4);
    }

    #[test]
    fn packet_wraps_around_cycle() {
        let base_seq = u16::MAX;
        let mut reporter = RTPReporter::new(base_seq);
        reporter.update_seq(1).unwrap();
        let expected_cycles: u32 = u16::MAX as u32 + 1;

        assert_eq!(reporter.received, 2);
        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, 1);
        assert_eq!(reporter.cycles, expected_cycles);
    }

    #[test]
    fn packet_exceeds_max_dropout() {
        let base_seq = 1;
        let mut reporter = RTPReporter::new(1);
        // Feed 2 packets
        reporter.update_seq(2).unwrap();
        reporter.update_seq(3).unwrap();

        let seq_exceeding_dropout_zone = MAX_DROPOUT + 3;

        // Make huge packet jump
        let expected_bad_seq = seq_exceeding_dropout_zone + 1;
        assert_eq!(reporter.update_seq(seq_exceeding_dropout_zone).unwrap_err(), UpdateSequenceError::InvalidSequence);

        // Check if bad_seq is updated
        assert_eq!(reporter.received, 3);
        assert_eq!(reporter.max_seq, 3);
        assert_eq!(reporter.base_seq, 1);
        assert_eq!(reporter.bad_seq, expected_bad_seq as u32);

        // Feed next packet in new seq order
        reporter.update_seq(expected_bad_seq).unwrap();

        // Reporter should restart at next sequential packet
        assert_eq!(reporter.base_seq, expected_bad_seq as u32);
        assert_eq!(reporter.max_seq, expected_bad_seq);
        assert_eq!(reporter.bad_seq, RTP_SEQ_MOD + 1);
        assert_eq!(reporter.received, 2);
    }

    #[test]
    fn lost_packet_is_reported() {
        let mut reporter = RTPReporter::new(1);
        reporter.update_seq(3).unwrap();

        assert_eq!(reporter.lost_packets, HashSet::from([2]))
    }

    #[test]
    fn multiple_lost_packets_in_cycle_are_reported() {
        let mut reporter = RTPReporter::new(1);
        reporter.update_seq(6).unwrap();

        assert_eq!(reporter.lost_packets, HashSet::from([2, 3, 4, 5]))
    }

    #[test]
    fn multiple_lost_packets_across_cycles_are_reported() {
        let mut reporter = RTPReporter::new(u16::MAX - 3);
        reporter.update_seq(2).unwrap();

        assert_eq!(reporter.lost_packets, HashSet::from([u16::MAX - 2, u16::MAX - 1, u16::MAX, 0, 1]))
    }

    #[test]
    fn lost_packet_across_cycle_is_reported() {
        let mut reporter = RTPReporter::new(u16::MAX);
        reporter.update_seq(1).unwrap();

        assert_eq!(reporter.lost_packets, HashSet::from([0]))
    }

    #[test]
    fn lost_packets_are_evicted_when_arrive_out_of_order() {
        let mut reporter = RTPReporter::new(1);
        reporter.update_seq(3).unwrap();
        assert_eq!(reporter.lost_packets, HashSet::from([2]));
        reporter.update_seq(2).unwrap();
        assert_eq!(reporter.lost_packets, HashSet::from([]));
    }
}

#[cfg(test)]
mod new {
    use std::collections::HashSet;
    use crate::rtp_reporter::{RTP_SEQ_MOD, RTPReporter};

    #[test]
    fn reporter_is_initialized_properly() {
        let input_seq = 2;
        let reporter = RTPReporter::new(input_seq);
        assert_eq!(reporter, RTPReporter {
            base_seq: input_seq as u32,
            max_seq: input_seq,
            received: 1,
            bad_seq: RTP_SEQ_MOD + 1,
            cycles: 0,
            lost_packets: HashSet::new(),
            expected_prior: 0,
            received_prior: 0,
        })
    }
}

#[cfg(test)]
mod lost_packets {
    use crate::rtp_reporter::RTPReporter;

    #[test]
    fn one_packet_received_in_total() {
        let mut reporter = RTPReporter::new(1);
        assert_eq!(reporter.lost_packets(), 0)
    }

    #[test]
    fn zero_packets_lost() {
        let mut reporter = RTPReporter::new(1);
        // Feed reporter some packets
        reporter.update_seq(2).unwrap();
        reporter.update_seq(3).unwrap();
        reporter.update_seq(4).unwrap();

        assert_eq!(reporter.lost_packets(), 0)
    }

    #[test]
    fn two_packets_lost() {
        let mut reporter = RTPReporter::new(1);
        // Feed reporter some packets
        reporter.update_seq(4).unwrap();

        assert_eq!(reporter.lost_packets(), 2)
    }

    #[test]
    fn three_packets_lost_when_wrapping() {
        let mut reporter = RTPReporter::new(u16::MAX - 1);
        // Feed reporter some packets
        reporter.update_seq(2).unwrap();

        assert_eq!(reporter.lost_packets(), 3)
    }

    #[test]
    fn two_packets_lost_and_one_recovered() {
        let mut reporter = RTPReporter::new(1);
        // Feed reporter some packets
        reporter.update_seq(5).unwrap();
        assert_eq!(reporter.lost_packets(), 3);

        // Feed one missing packet
        reporter.update_seq(2).unwrap();
        assert_eq!(reporter.lost_packets(), 2)
    }
}