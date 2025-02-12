use std::collections::HashSet;

#[derive(Debug)]
pub struct Reporter {
    pub ext_highest_seq: usize,
    pub lost_packets: HashSet<usize>,
    packet_loss_sum: usize,
    roc: usize,
}

impl Reporter {
    pub fn process_packet(&mut self, pid: usize, roc: usize) {
        // Packet crosses the roc counter boundary
        if roc != self.roc {
            // Packet is the next packet to process
            if roc > self.roc {
                let packets_lost_since_last_roc = (u16::MAX as usize) - self.ext_highest_seq;
                let packets_lost_in_new_roc = pid;

                let mut index = 1;
                let mut packets_lost = HashSet::new();

                while index <= packets_lost_since_last_roc {
                    packets_lost.insert(self.ext_highest_seq + index);
                    index += 1;
                }
                index = 0;
                while index < packets_lost_in_new_roc {
                    packets_lost.insert(index);
                    index += 1;
                }
                self.roc = roc;
                self.ext_highest_seq = pid;
                self.lost_packets = self.lost_packets.union(&packets_lost).map(ToOwned::to_owned).collect::<HashSet<usize>>();
            }
            // Old packet was re-transmitted
            else {
                self.lost_packets.remove(&pid);
            }
        }

        // Packet comes in order
        else if pid == self.ext_highest_seq + 1 {
            self.ext_highest_seq = pid;
            self.roc = roc;
        }
        // Packets comes ahead of order but in sliding-window boundary
        else if pid > self.ext_highest_seq + 1 {
            let mut packets_lost = pid - self.ext_highest_seq - 1;
            let mut lost_packets = HashSet::new();

            while packets_lost > 0 {
                lost_packets.insert(self.ext_highest_seq + packets_lost);
                packets_lost -= 1;
            }

            self.lost_packets = self.lost_packets.union(&lost_packets).map(ToOwned::to_owned).collect::<HashSet<usize>>();
            self.ext_highest_seq = pid;
        }
        // Packet is late/duplicate/re-send
        else {
            self.lost_packets.remove(&pid);
        }
    }

    pub fn new(pid: usize, roc: usize) -> Self {
        Self {
            roc,
            ext_highest_seq: pid,
            lost_packets: HashSet::new(),
            packet_loss_sum: 0,
        }
    }
}

impl Default for Reporter {
    fn default() -> Self {
        Self {
            roc: 0,
            packet_loss_sum: 0,
            ext_highest_seq: 0,
            lost_packets: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod reporters_tests {
    use std::collections::HashSet;
    use crate::rtcp_reporter::Reporter;

    #[test]
    fn packets_come_in_order() {
        let mut reporter = Reporter::default();

        reporter.process_packet(1, 0);
        reporter.process_packet(2, 0);
        reporter.process_packet(3, 0);
        reporter.process_packet(4, 0);

        assert_eq!(reporter.lost_packets.len(), 0);
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 0);
        assert_eq!(reporter.ext_highest_seq, 4);
    }

    #[test]
    fn packets_come_out_of_order() {
        let mut reporter = Reporter::default();

        reporter.process_packet(1, 0);
        reporter.process_packet(4, 0);
        reporter.process_packet(2, 0);
        reporter.process_packet(3, 0);


        assert_eq!(reporter.lost_packets.len(), 0);
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 0);
        assert_eq!(reporter.ext_highest_seq, 4);
    }

    #[test]
    fn one_packet_is_lost() {
        let mut reporter = Reporter::default();

        reporter.process_packet(1, 0);
        reporter.process_packet(4, 0);
        reporter.process_packet(3, 0);


        assert_eq!(reporter.lost_packets, HashSet::from([2]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 0);
        assert_eq!(reporter.ext_highest_seq, 4);
    }

    #[test]
    fn two_packets_are_lost() {
        let mut reporter = Reporter::default();

        reporter.process_packet(1, 0);
        reporter.process_packet(2, 0);
        reporter.process_packet(4, 0);
        reporter.process_packet(5, 0);
        reporter.process_packet(7, 0);


        assert_eq!(reporter.lost_packets, HashSet::from([3, 6]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 0);
        assert_eq!(reporter.ext_highest_seq, 7);
    }

    #[test]
    fn packets_arriving_late_evict_lost_packet() {
        let mut reporter = Reporter::default();

        reporter.process_packet(1, 0);
        reporter.process_packet(6, 0);
        reporter.process_packet(5, 0);
        reporter.process_packet(4, 0);
        reporter.process_packet(3, 0);
        reporter.process_packet(2, 0);


        assert_eq!(reporter.lost_packets, HashSet::from([]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 0);
        assert_eq!(reporter.ext_highest_seq, 6);
    }

    #[test]
    fn packet_jumps_roc() {
        let mut reporter = Reporter {
            lost_packets: HashSet::new(),
            ext_highest_seq: u16::MAX as usize,
            roc: 0,
            packet_loss_sum: 0,
        };

        reporter.process_packet(0, 1);
        reporter.process_packet(1, 1);


        assert_eq!(reporter.lost_packets, HashSet::from([]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 1);
        assert_eq!(reporter.ext_highest_seq, 1);
    }

    #[test]
    fn packet_jumps_roc_and_loses_packet() {
        let mut reporter = Reporter {
            lost_packets: HashSet::new(),
            ext_highest_seq: u16::MAX as usize,
            roc: 0,
            packet_loss_sum: 0,
        };

        reporter.process_packet(5, 1);


        assert_eq!(reporter.lost_packets, HashSet::from([0, 1, 2, 3, 4]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 1);
        assert_eq!(reporter.ext_highest_seq, 5);
    }

    #[test]
    fn packet_jumps_roc_and_loses_packet_before_counter_increment() {
        let mut reporter = Reporter {
            lost_packets: HashSet::new(),
            ext_highest_seq: (u16::MAX as usize) - 4,
            roc: 0,
            packet_loss_sum: 0,
        };
        reporter.process_packet((u16::MAX as usize) - 3, 0);
        reporter.process_packet(0, 1);


        assert_eq!(reporter.lost_packets, HashSet::from([u16::MAX as usize, u16::MAX as usize - 1, u16::MAX as usize - 2]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 1);
        assert_eq!(reporter.ext_highest_seq, 0);
    }

    #[test]
    fn packet_jumps_roc_and_loses_packets_before_and_after_roc_increment() {
        let mut reporter = Reporter {
            lost_packets: HashSet::new(),
            ext_highest_seq: u16::MAX as usize - 4,
            roc: 0,
            packet_loss_sum: 0,
        };
        reporter.process_packet(u16::MAX as usize - 3, 0);
        reporter.process_packet(5, 1);


        assert_eq!(reporter.lost_packets, HashSet::from([u16::MAX as usize, u16::MAX as usize - 1, u16::MAX as usize - 2, 4, 3, 2, 1, 0]));
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 1);
        assert_eq!(reporter.ext_highest_seq, 5);
    }

    #[test]
    fn packet_jumps_roc_out_of_order() {
        let mut reporter = Reporter {
            lost_packets: HashSet::new(),
            ext_highest_seq: u16::MAX as usize - 4,
            roc: 0,
            packet_loss_sum: 0,
        };
        reporter.process_packet(u16::MAX as usize - 3, 0);
        reporter.process_packet(2, 1);
        reporter.process_packet(u16::MAX as usize - 2, 0);
        reporter.process_packet(u16::MAX as usize - 1, 0);
        reporter.process_packet(0, 1);
        reporter.process_packet(1, 1);
        reporter.process_packet(u16::MAX as usize, 0);


        assert_eq!(reporter.lost_packets.len(), 0);
        assert_eq!(reporter.packet_loss_sum, 0);
        assert_eq!(reporter.roc, 1);
        assert_eq!(reporter.ext_highest_seq, 2);
    }
}