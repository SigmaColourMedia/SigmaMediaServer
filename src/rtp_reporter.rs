use std::collections::HashSet;
use bytes::Bytes;
use rtcp::receiver_report::{ReceiverReport, ReportBlock};
use rtcp::sdes::SourceDescriptor;
use rtcp::transport_layer_feedback::GenericNACK;

#[derive(Debug, Clone, PartialEq)]
struct RTPReporter {
    max_seq: u16,
    cycles: u32,
    base_seq: u32,
    bad_seq: u32,
    received: u32,
    expected_prior: u32,
    received_prior: u32,
    missing_packets: HashSet<u16>,
    host_ssrc: u32,
    media_ssrc: u32,
    dlsr: u32,
    lsr: u32,
}

#[derive(Debug, Clone, PartialEq)]
enum UpdateSequenceError {
    InvalidSequence
}


impl RTPReporter {
    fn new(seq: u16, host_ssrc: u32, media_ssrc: u32) -> Self {
        Self {
            cycles: 0,
            max_seq: seq,
            base_seq: seq as u32,
            bad_seq: RTP_SEQ_MOD + 1,
            received: 1,
            received_prior: 0,
            expected_prior: 0,
            missing_packets: HashSet::new(),
            dlsr: 0,
            lsr: 0,
            host_ssrc,
            media_ssrc,
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
                    self.missing_packets.insert(packet + self.max_seq + 1);
                }
                // Add any missing packets in new cycle
                let packets_missed_in_new_cycle = seq;
                for packet in 0..packets_missed_in_new_cycle {
                    self.missing_packets.insert(packet);
                }
                //  count another cycle.
                self.cycles += RTP_SEQ_MOD;
            }
            // In cycle, jumped few packets forwards
            else {
                // Add any missing packets in current cycle
                let packets_missed_in_cycle = seq - self.max_seq - 1;
                for packet in 0..packets_missed_in_cycle {
                    self.missing_packets.insert(packet + self.max_seq + 1);
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
                let _ = std::mem::replace(self, RTPReporter::new(seq, self.host_ssrc, self.media_ssrc));
                // Bad packet, await for next sequential packet
            } else {
                self.bad_seq = (seq as u32 + 1) & (RTP_SEQ_MOD - 1);
                return Err(UpdateSequenceError::InvalidSequence);
            }
            /* duplicate or reordered packet */
        } else {
            // Evict lost packet
            self.missing_packets.remove(&seq);
        }
        self.received += 1;
        Ok(())
    }

    fn lost_packets(&self) -> u32 {
        let extended_max = self.cycles + self.max_seq as u32;
        let expected = extended_max - self.base_seq + 1;
        expected - self.received
    }

    fn fraction_lost(&mut self) -> u8 {
        let extended_max = self.cycles + self.max_seq as u32;
        let expected = extended_max - self.base_seq + 1;
        let expected_interval = expected - self.expected_prior;
        self.expected_prior = expected;

        let received_interval = self.received - self.received_prior;
        self.received_prior = self.received;

        let lost_interval = expected_interval.wrapping_sub(received_interval);
        if expected_interval == 0 || lost_interval <= 0 {
            return 0;
        }
        return ((lost_interval << 8) as u32 / expected_interval) as u8;
    }

    fn cleanup_stale_missing_packets(&mut self) {
        let max_seq = self.max_seq;


        self.missing_packets = self.missing_packets.drain().filter(|item| {
            let delta = item.wrapping_sub(max_seq);
            let is_stale = delta as u32 <= RTP_SEQ_MOD - MAX_MISORDER;
            !is_stale
        }).collect::<HashSet<u16>>();
    }

    fn generate_receiver_report(&mut self) -> Bytes {
        let report_block = ReportBlock {
            ext_highest_sequence: self.max_seq as u32,
            fraction_lost: self.fraction_lost(),
            ssrc: self.media_ssrc,
            jitter: 0, // Unsupported
            cumulative_packets_lost: self.lost_packets(),
            dlsr: self.dlsr,
            lsr: self.lsr,
        };

        let receiver_report = ReceiverReport::new(self.host_ssrc, vec![report_block]);
        self.cleanup_stale_missing_packets();

        let nacks = generate_nacks(&mut self.missing_packets);

        Bytes::new()
    }
}

fn generate_nacks(missing_packets: &HashSet<u16>) -> Vec<GenericNACK> {
    let mut packets = missing_packets.iter().collect::<Vec<&u16>>();
    packets.sort();

    let mut stack: Vec<(u16, Vec<u16>)> = vec![];

    for packet in packets {
        if let Some((curr_id, packet_vec)) = stack.last_mut() {
            if packet - *curr_id > 16 {
                stack.push((*packet, vec![]));
            } else {
                packet_vec.push(*packet)
            }
        } else {
            stack.push((*packet, vec![]));
        }
    }

    stack.into_iter().map(|(id, other)| map_to_nack(id, other)).collect::<Vec<GenericNACK>>()
}

fn map_to_nack(pid: u16, next_pids: Vec<u16>) -> GenericNACK {
    let mut blp: u16 = 0b0000_0000_0000_0000;
    for packet in next_pids {
        let index = packet - pid;
        match index {
            1 => blp = blp ^ 0b0000_0000_0000_0001,
            2 => blp = blp ^ 0b0000_0000_0000_0010,
            3 => blp = blp ^ 0b0000_0000_0000_0100,
            4 => blp = blp ^ 0b0000_0000_0000_1000,
            5 => blp = blp ^ 0b0000_0000_0001_0000,
            6 => blp = blp ^ 0b0000_0000_0010_0000,
            7 => blp = blp ^ 0b0000_0000_0100_0000,
            8 => blp = blp ^ 0b0000_0000_1000_0000,
            9 => blp = blp ^ 0b0000_0001_0000_0000,
            10 => blp = blp ^ 0b0000_0010_0000_0000,
            11 => blp = blp ^ 0b0000_0100_0000_0000,
            12 => blp = blp ^ 0b0000_1000_0000_0000,
            13 => blp = blp ^ 0b0001_0000_0000_0000,
            14 => blp = blp ^ 0b0010_0000_0000_0000,
            15 => blp = blp ^ 0b0100_0000_0000_0000,
            16 => blp = blp ^ 0b1000_0000_0000_0000,
            _ => panic!("Should only include packets with id pid < packet_id <= pid + 16")
        }
    };
    GenericNACK {
        pid,
        blp,
    }
}


static MAX_DROPOUT: u16 = 3000;
static MAX_MISORDER: u32 = 180;
static RTP_SEQ_MOD: u32 = 1 << 16;

#[cfg(test)]
mod generate_nacks {
    use std::collections::HashSet;
    use rtcp::transport_layer_feedback::GenericNACK;
    use crate::rtp_reporter::generate_nacks;

    #[test]
    fn generate_one_nack() {
        let input = HashSet::from([4, 2, 0, 6, 10]);

        let output = generate_nacks(&input);

        assert_eq!(output, vec![GenericNACK {
            pid: 0,
            blp: 0b0000_0010_0010_1010,
        }])
    }

    #[test]
    fn generate_two_nack() {
        let input = HashSet::from([4, 2, 0, 6, 10, 16, 18, 20, 35, 37]);

        let output = generate_nacks(&input);

        assert_eq!(output, vec![
            GenericNACK {
                pid: 0,
                blp: 0b1000_0010_0010_1010,
            },
            GenericNACK {
                pid: 18,
                blp: 0b0000_0000_0000_0010,
            },
            GenericNACK {
                pid: 35,
                blp: 0b0000_0000_0000_0010,
            },
        ]);
    }

    #[test]
    fn two_nacks_spanning_cycle() {
        let input = HashSet::from([4, 2, 0, 6, 10, 16, u16::MAX, u16::MAX - 1]);
        let output = generate_nacks(&input);
        assert_eq!(output, vec![
            GenericNACK {
                pid: 0,
                blp: 0b1000_0010_0010_1010,
            },
            GenericNACK {
                pid: u16::MAX - 1,
                blp: 0b0000_0000_0000_0001,
            },
        ]);
    }


    #[test]
    fn generate_three_nacks() {
        let input = HashSet::from([4, 2, 0, 6, 10, 18, 20]);

        let output = generate_nacks(&input);

        assert_eq!(output, vec![
            GenericNACK {
                pid: 0,
                blp: 0b0000_0010_0010_1010,
            },
            GenericNACK {
                pid: 18,
                blp: 0b0000_0000_0000_0010,
            }])
    }
}

#[cfg(test)]
mod map_to_nack {
    use rtcp::transport_layer_feedback::GenericNACK;
    use crate::rtp_reporter::map_to_nack;

    #[test]
    fn zero_extra_packets() {
        let output = map_to_nack(0, vec![]);

        assert_eq!(output, GenericNACK {
            pid: 0,
            blp: 0,
        })
    }

    #[test]
    fn next_packet_included() {
        let output = map_to_nack(0, vec![1]);

        assert_eq!(output, GenericNACK {
            pid: 0,
            blp: 0b0000_0000_0000_0001,
        })
    }

    #[test]
    fn third_and_fifth_packets_included() {
        let output = map_to_nack(0, vec![3, 5]);

        assert_eq!(output, GenericNACK {
            pid: 0,
            blp: 0b0000_0000_0001_0100,
        })
    }

    #[test]
    fn tenth_and_seventh_and_fifth_packets_included() {
        let output = map_to_nack(0, vec![10, 5, 7]);

        assert_eq!(output, GenericNACK {
            pid: 0,
            blp: 0b0000_0010_0101_0000,
        })
    }

    #[test]
    fn all_packets_included() {
        let output = map_to_nack(0, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);

        assert_eq!(output, GenericNACK {
            pid: 0,
            blp: 0b1111_1111_1111_1111,
        })
    }
}

#[cfg(test)]
mod cleanup_stale_missing_packets {
    use std::collections::HashSet;
    use crate::rtp_reporter::{MAX_MISORDER, RTP_SEQ_MOD, RTPReporter};

    #[test]
    fn evicts_stale_packet() {
        let max_seq: u16 = 5003;
        let max_disorder = MAX_MISORDER as u16;

        let mut reporter = RTPReporter {
            missing_packets: HashSet::from([
                max_seq - max_disorder, // Stale packet
                max_seq - max_disorder + 1, max_seq - max_disorder + 2, max_seq - max_disorder + 3 // Packets within limit
            ]),
            max_seq,
            media_ssrc: 1,
            host_ssrc: 0,
            base_seq: 0,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            expected_prior: 0,
            received: 5000,
            received_prior: 0,
            lsr: 0,
            dlsr: 0,
        };

        reporter.cleanup_stale_missing_packets();

        assert_eq!(reporter.missing_packets, HashSet::from([max_seq - max_disorder + 1, max_seq - max_disorder + 2, max_seq - max_disorder + 3]))
    }

    #[test]
    fn does_not_discard_packets_within_max_disorder_boundary() {
        let max_seq: u16 = 5003;
        let max_disorder = MAX_MISORDER as u16;

        let mut reporter = RTPReporter {
            missing_packets: HashSet::from([
                max_seq - max_disorder + 1, max_seq - max_disorder + 2, max_seq - max_disorder + 3 // Packets within limit
            ]),
            max_seq,
            media_ssrc: 1,
            host_ssrc: 0,
            base_seq: 0,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            expected_prior: 0,
            received: 5000,
            received_prior: 0,
            lsr: 0,
            dlsr: 0,
        };

        reporter.cleanup_stale_missing_packets();

        assert_eq!(reporter.missing_packets, HashSet::from([max_seq - max_disorder + 1, max_seq - max_disorder + 2, max_seq - max_disorder + 3]))
    }

    #[test]
    fn discards_multiple_stale_packets() {
        let max_seq: u16 = 5003;
        let max_disorder = MAX_MISORDER as u16;

        let mut reporter = RTPReporter {
            missing_packets: HashSet::from([
                max_seq - max_disorder - 20, max_seq - max_disorder - 1, max_seq - max_disorder, // Stale packets
                max_seq - max_disorder + 1, max_seq - max_disorder + 2, max_seq - max_disorder + 3 // Packets within limit
            ]),
            max_seq,
            media_ssrc: 1,
            host_ssrc: 0,
            base_seq: 0,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            expected_prior: 0,
            received: 5000,
            received_prior: 0,
            lsr: 0,
            dlsr: 0,
        };

        reporter.cleanup_stale_missing_packets();

        assert_eq!(reporter.missing_packets, HashSet::from([max_seq - max_disorder + 1, max_seq - max_disorder + 2, max_seq - max_disorder + 3]))
    }

    #[test]
    fn keeps_packets_crossing_cycle_boundary_within_max_disorder() {
        let max_seq: u16 = 3;
        let max_disorder = MAX_MISORDER as u16;

        let mut reporter = RTPReporter {
            missing_packets: HashSet::from([
                u16::MAX - 3, u16::MAX - 2, u16::MAX, // Packets in prev cycle
                max_seq - 1, max_seq - 2 // Packets in new cycle
            ]),
            max_seq,
            media_ssrc: 1,
            host_ssrc: 0,
            base_seq: 0,
            cycles: RTP_SEQ_MOD, // We're in the second cycle
            bad_seq: RTP_SEQ_MOD + 1,
            expected_prior: 0,
            received: 5000,
            received_prior: 0,
            lsr: 0,
            dlsr: 0,
        };

        reporter.cleanup_stale_missing_packets();

        assert_eq!(reporter.missing_packets, HashSet::from([max_seq - 2, max_seq - 1, u16::MAX - 3, u16::MAX - 2, u16::MAX]))
    }

    #[test]
    fn discards_packets_crossing_cycle_boundary_past_max_disorder() {
        let max_seq = MAX_MISORDER as u16;

        let mut reporter = RTPReporter {
            missing_packets: HashSet::from([u16::MAX - 3, u16::MAX - 2, MAX_MISORDER as u16 - 1]),
            max_seq,
            media_ssrc: 1,
            host_ssrc: 0,
            base_seq: 0,
            cycles: RTP_SEQ_MOD, // We're in the second cycle
            bad_seq: RTP_SEQ_MOD + 1,
            expected_prior: 0,
            received: 5000,
            received_prior: 0,
            lsr: 0,
            dlsr: 0,
        };

        reporter.cleanup_stale_missing_packets();

        assert_eq!(reporter.missing_packets, HashSet::from([MAX_MISORDER as u16 - 1]))
    }
}

#[cfg(test)]
mod fraction_lost {
    use std::collections::HashSet;
    use crate::rtp_reporter::{RTP_SEQ_MOD, RTPReporter};

    #[test]
    fn no_packets_lost() {
        let mut reporter = RTPReporter::new(1, 0, 1);

        let lost = reporter.fraction_lost();
        let extended_max = reporter.cycles + reporter.max_seq as u32;
        let expected = extended_max - reporter.base_seq + 1;
        assert_eq!(lost, 0);

        // Should update prior
        assert_eq!(reporter.received_prior, reporter.received);
        assert_eq!(reporter.expected_prior, expected);
    }

    #[test]
    fn half_packets_lost_since_last_report() {
        let mut reporter = RTPReporter {
            missing_packets: HashSet::new(),
            received_prior: 4,
            received: 4,
            expected_prior: 4,
            max_seq: 4,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            base_seq: 1,
            host_ssrc: 0,
            media_ssrc: 1,
            lsr: 0,
            dlsr: 0,
        };

        reporter.update_seq(6).unwrap();


        let lost = reporter.fraction_lost();
        let percentage = lost as f32 / 256.0;
        assert_eq!(percentage, 0.5);

        // Should update prior
        let extended_max = reporter.cycles + reporter.max_seq as u32;
        let expected = extended_max - reporter.base_seq + 1;
        assert_eq!(reporter.received_prior, reporter.received);
        assert_eq!(reporter.expected_prior, expected);
    }

    #[test]
    fn quarter_packets_lost_since_last_report() {
        let mut reporter = RTPReporter {
            missing_packets: HashSet::new(),
            received_prior: 4,
            received: 4,
            expected_prior: 4,
            max_seq: 4,
            cycles: 0,
            bad_seq: RTP_SEQ_MOD + 1,
            base_seq: 1,
            host_ssrc: 0,
            media_ssrc: 1,
            lsr: 0,
            dlsr: 0,
        };

        reporter.update_seq(5).unwrap();
        reporter.update_seq(6).unwrap();
        reporter.update_seq(8).unwrap();


        let lost = reporter.fraction_lost();
        let percentage = lost as f32 / 256.0;
        assert_eq!(percentage, 0.25);

        // Should update prior
        let extended_max = reporter.cycles + reporter.max_seq as u32;
        let expected = extended_max - reporter.base_seq + 1;
        assert_eq!(reporter.received_prior, reporter.received);
        assert_eq!(reporter.expected_prior, expected);
    }
}

#[cfg(test)]
mod update_seq {
    use std::collections::HashSet;
    use crate::rtp_reporter::{MAX_DROPOUT, RTP_SEQ_MOD, RTPReporter, UpdateSequenceError};

    #[test]
    fn packet_comes_in_order() {
        let base_seq = 2;
        let mut reporter = RTPReporter::new(base_seq, 0, 1);
        let next_seq = 3;
        let result = reporter.update_seq(next_seq).unwrap();

        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, next_seq);
        assert_eq!(reporter.received, 2);
    }

    #[test]
    fn packet_skips_3_seq() {
        let base_seq = 2;
        let mut reporter = RTPReporter::new(base_seq, 0, 1);
        let next_seq = 6;
        let result = reporter.update_seq(next_seq).unwrap();

        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, next_seq);
        assert_eq!(reporter.received, 2);
    }

    #[test]
    fn reordered_packet_comes_in() {
        let base_seq = 2;
        let mut reporter = RTPReporter::new(base_seq, 0, 1);
        reporter.update_seq(4).unwrap();
        reporter.update_seq(3).unwrap();

        assert_eq!(reporter.received, 3);
        assert_eq!(reporter.base_seq, base_seq as u32);
        assert_eq!(reporter.max_seq, 4);
    }

    #[test]
    fn packet_wraps_around_cycle() {
        let base_seq = u16::MAX;
        let mut reporter = RTPReporter::new(base_seq, 0, 1);
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
        let mut reporter = RTPReporter::new(1, 0, 1);
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
        let mut reporter = RTPReporter::new(1, 0, 1);
        reporter.update_seq(3).unwrap();

        assert_eq!(reporter.missing_packets, HashSet::from([2]))
    }

    #[test]
    fn multiple_lost_packets_in_cycle_are_reported() {
        let mut reporter = RTPReporter::new(1, 0, 1);
        reporter.update_seq(6).unwrap();

        assert_eq!(reporter.missing_packets, HashSet::from([2, 3, 4, 5]))
    }

    #[test]
    fn multiple_lost_packets_across_cycles_are_reported() {
        let mut reporter = RTPReporter::new(u16::MAX - 3, 0, 1);
        reporter.update_seq(2).unwrap();

        assert_eq!(reporter.missing_packets, HashSet::from([u16::MAX - 2, u16::MAX - 1, u16::MAX, 0, 1]))
    }

    #[test]
    fn lost_packet_across_cycle_is_reported() {
        let mut reporter = RTPReporter::new(u16::MAX, 0, 1);
        reporter.update_seq(1).unwrap();

        assert_eq!(reporter.missing_packets, HashSet::from([0]))
    }

    #[test]
    fn lost_packets_are_evicted_when_arrive_out_of_order() {
        let mut reporter = RTPReporter::new(1, 0, 1);
        reporter.update_seq(3).unwrap();
        assert_eq!(reporter.missing_packets, HashSet::from([2]));
        reporter.update_seq(2).unwrap();
        assert_eq!(reporter.missing_packets, HashSet::from([]));
    }
}

#[cfg(test)]
mod new {
    use std::collections::HashSet;
    use crate::rtp_reporter::{RTP_SEQ_MOD, RTPReporter};

    #[test]
    fn reporter_is_initialized_properly() {
        let input_seq = 2;
        let reporter = RTPReporter::new(input_seq, 0, 1);
        assert_eq!(reporter, RTPReporter {
            base_seq: input_seq as u32,
            max_seq: input_seq,
            received: 1,
            bad_seq: RTP_SEQ_MOD + 1,
            cycles: 0,
            missing_packets: HashSet::new(),
            expected_prior: 0,
            received_prior: 0,
            host_ssrc: 0,
            media_ssrc: 1,
            lsr: 0,
            dlsr: 0,
        })
    }
}

#[cfg(test)]
mod lost_packets {
    use crate::rtp_reporter::RTPReporter;

    #[test]
    fn one_packet_received_in_total() {
        let mut reporter = RTPReporter::new(1, 0, 1);
        assert_eq!(reporter.lost_packets(), 0)
    }

    #[test]
    fn zero_packets_lost() {
        let mut reporter = RTPReporter::new(1, 0, 1);
        // Feed reporter some packets
        reporter.update_seq(2).unwrap();
        reporter.update_seq(3).unwrap();
        reporter.update_seq(4).unwrap();

        assert_eq!(reporter.lost_packets(), 0)
    }

    #[test]
    fn two_packets_lost() {
        let mut reporter = RTPReporter::new(1, 0, 1);
        // Feed reporter some packets
        reporter.update_seq(4).unwrap();

        assert_eq!(reporter.lost_packets(), 2)
    }

    #[test]
    fn three_packets_lost_when_wrapping() {
        let mut reporter = RTPReporter::new(u16::MAX - 1, 0, 1);
        // Feed reporter some packets
        reporter.update_seq(2).unwrap();

        assert_eq!(reporter.lost_packets(), 3)
    }

    #[test]
    fn two_packets_lost_and_one_recovered() {
        let mut reporter = RTPReporter::new(1, 0, 1);
        // Feed reporter some packets
        reporter.update_seq(5).unwrap();
        assert_eq!(reporter.lost_packets(), 3);

        // Feed one missing packet
        reporter.update_seq(2).unwrap();
        assert_eq!(reporter.lost_packets(), 2)
    }
}