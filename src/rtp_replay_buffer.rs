use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use bytes::{Buf, Bytes};
use tokio::time::Instant;

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

    pub fn insert(&mut self, packet: Bytes) {
        let should_evict = self.heap.len() == self.size;
        if should_evict {
            let oldest = self.heap.pop().expect("Heap should not be empty");
            self.map.remove(&oldest.seq);
        }
        let seq = packet.slice(2..).get_u16();

        let packet_id = RTPPacketID {
            seq,
            time: Instant::now(),
        };
        self.heap.push(packet_id);
        self.map.insert(packet_id.seq, packet);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
struct RTPPacketID {
    seq: u16,
    time: Instant,
}

impl PartialOrd for RTPPacketID {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RTPPacketID {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.time > other.time {
            Ordering::Less
        } else if self.time < self.time {
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

    use crate::rtp_replay_buffer::ReplayBuffer;

    #[test]
    fn inserts_in_ok_order() {
        let mut replay_buffer = ReplayBuffer {
            size: 3,
            map: HashMap::new(),
            heap: BinaryHeap::new(),
        };

        replay_buffer.insert(Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 10, // SEQ number
                128, 128,
            ], // Payload suffix
        ));
        replay_buffer.insert(Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 20, // SEQ number
                128, 128,
            ], // Payload suffix
        ));
        replay_buffer.insert(Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 4, // SEQ number
                128, 128,
            ], // Payload suffix
        ));

        assert_eq!(replay_buffer.heap.pop().unwrap().seq, 10);
        assert_eq!(replay_buffer.heap.pop().unwrap().seq, 20);
        assert_eq!(replay_buffer.heap.pop().unwrap().seq, 4);
        assert_eq!(replay_buffer.heap.pop(), None);
    }

    #[test]
    fn evicts_overflow_members() {
        let mut replay_buffer = ReplayBuffer {
            size: 3,
            map: HashMap::new(),
            heap: BinaryHeap::new(),
        };

        let packet_0 = (Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 10, // SEQ number
                128, 128,
            ], // Payload suffix
        ),);
        let packet_1 = (Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 20, // SEQ number
                128, 128,
            ], // Payload suffix
        ),);

        let packet_2 = (Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 4, // SEQ number
                128, 128,
            ], // Payload suffix
        ),);
        let packet_3 = (Bytes::from_static(
            &[
                0, 0, // Prefix
                0, 5, // SEQ number
                128, 128,
            ], // Payload suffix
        ),);

        replay_buffer.insert(packet_0.0.clone());
        replay_buffer.insert(packet_1.0.clone());
        replay_buffer.insert(packet_2.0.clone());
        assert_eq!(replay_buffer.map.get(&10), Some(&packet_0.0));

        replay_buffer.insert(packet_3.0.clone());

        // Test internal heap
        let mut sorted_heap = replay_buffer.heap.clone().into_sorted_vec();
        sorted_heap.reverse();

        // Test internal HashMap
        assert_eq!(replay_buffer.map.len(), 3);
        assert_eq!(replay_buffer.map.get(&10), None);
        assert_eq!(replay_buffer.map.get(&20), Some(&packet_1.0.clone()));
        assert_eq!(replay_buffer.map.get(&4), Some(&packet_2.0.clone()));
        assert_eq!(replay_buffer.map.get(&5), Some(&packet_3.0.clone()));

        // Test Public get API
        assert_eq!(replay_buffer.get(20), Some(&packet_1.0.clone()));
        assert_eq!(replay_buffer.get(4), Some(&packet_2.0.clone()));
        assert_eq!(replay_buffer.get(5), Some(&packet_3.0.clone()));
    }
}
