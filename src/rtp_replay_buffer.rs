use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use bytes::{Buf, Bytes};

static DEFAULT_RTP_REPLAY_BUFF_SIZE: usize = 1024;


#[derive(Debug)]
pub(crate) struct ReplayBuffer {
    heap: BinaryHeap<RTPPacketID>,
    map: HashMap<u16, Bytes>,
    size: usize,
}


impl Default for ReplayBuffer {
    fn default() -> Self {
        Self {
            size: DEFAULT_RTP_REPLAY_BUFF_SIZE,
            heap: BinaryHeap::new(),
            map: HashMap::new(),
        }
    }
}

impl ReplayBuffer {
    pub fn get(&self, id: u16) -> Option<&Bytes> {
        self.map.get(&id)
    }

    pub fn insert(&mut self, packet: Bytes, roc: u32) {
        let should_evict = self.heap.len() == self.size;
        if should_evict {
            let oldest = self.heap.pop().expect("Heap should not be empty");
            self.map.remove(&oldest.seq);
        }
        let seq = packet.slice(2..).get_u16();

        let packet_id = RTPPacketID { seq, roc };
        self.heap.push(packet_id);
        self.map.insert(packet_id.seq, packet);
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
struct RTPPacketID {
    seq: u16,
    roc: u32,
}


impl PartialOrd for RTPPacketID {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RTPPacketID {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_val = self.roc as usize * (u16::MAX as usize) + self.seq as usize;
        let other_val = other.roc as usize * (u16::MAX as usize) + other.seq as usize;

        if self_val > other_val {
            Ordering::Less
        } else if self_val < other_val {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}


#[cfg(test)]
mod rtp_replay_buffer {
    use std::collections::{BinaryHeap, HashMap};
    use bytes::Bytes;
    use crate::rtp_replay_buffer::{ReplayBuffer, RTPPacketID};

    #[test]
    fn inserts_in_ok_order() {
        let mut replay_buffer = ReplayBuffer {
            size: 3,
            map: HashMap::new(),
            heap: BinaryHeap::new(),
        };

        replay_buffer.insert(Bytes::from_static(&[
            0, 0, // Prefix
            0, 20, // SEQ number
            128, 128] // Payload suffix
        ), 0);
        replay_buffer.insert(Bytes::from_static(&[
            0, 0, // Prefix
            0, 10, // SEQ number
            128, 128] // Payload suffix
        ), 0);
        replay_buffer.insert(Bytes::from_static(&[
            0, 0, // Prefix
            0, 4, // SEQ number
            128, 128] // Payload suffix
        ), 1);

        assert_eq!(replay_buffer.heap.pop(), Some(RTPPacketID { roc: 0, seq: 10 }));
        assert_eq!(replay_buffer.heap.pop(), Some(RTPPacketID { roc: 0, seq: 20 }));
        assert_eq!(replay_buffer.heap.pop(), Some(RTPPacketID { roc: 1, seq: 4 }));
        assert_eq!(replay_buffer.heap.pop(), None);
    }

    #[test]
    fn evicts_overflow_members() {
        let mut replay_buffer = ReplayBuffer {
            size: 3,
            map: HashMap::new(),
            heap: BinaryHeap::new(),
        };

        let packet_0 = (Bytes::from_static(&[
            0, 0, // Prefix
            0, 10, // SEQ number
            128, 128] // Payload suffix
        ), 0);
        let packet_1 = (Bytes::from_static(&[
            0, 0, // Prefix
            0, 20, // SEQ number
            128, 128] // Payload suffix
        ), 0);

        let packet_2 = (Bytes::from_static(&[
            0, 0, // Prefix
            0, 4, // SEQ number
            128, 128] // Payload suffix
        ), 1);
        let packet_3 = (Bytes::from_static(&[
            0, 0, // Prefix
            0, 5, // SEQ number
            128, 128] // Payload suffix
        ), 1);

        replay_buffer.insert(packet_1.0.clone(), packet_1.1);

        replay_buffer.insert(packet_0.0.clone(), packet_0.1);
        replay_buffer.insert(packet_2.0.clone(), packet_2.1);
        replay_buffer.insert(packet_3.0.clone(), packet_3.1);


        // Test internal heap
        let mut sorted_heap = replay_buffer.heap.clone().into_sorted_vec();
        sorted_heap.reverse();

        assert_eq!(sorted_heap, vec![RTPPacketID {
            seq: 20,
            roc: 0,
        }, RTPPacketID {
            seq: 4,
            roc: 1,
        }, RTPPacketID {
            seq: 5,
            roc: 1,
        }]);

        // Test internal HashMap
        assert_eq!(replay_buffer.map.len(), 3);
        assert_eq!(replay_buffer.map.get(&20), Some(&packet_1.0.clone()));
        assert_eq!(replay_buffer.map.get(&4), Some(&packet_2.0.clone()));
        assert_eq!(replay_buffer.map.get(&5), Some(&packet_3.0.clone()));

        // Test Public get API
        assert_eq!(replay_buffer.get(20), Some(&packet_1.0.clone()));
        assert_eq!(replay_buffer.get(4), Some(&packet_2.0.clone()));
        assert_eq!(replay_buffer.get(5), Some(&packet_3.0.clone()));
    }
}


