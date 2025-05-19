use std::collections::HashMap;

use bytes::{Buf, Bytes};

pub struct Cache {
    map: HashMap<u16, usize>,
    items: Vec<Vec<u8>>,
    index: usize,
}

const BUFFER_SIZE: usize = 600;
impl Cache {
    pub fn new() -> Self {
        Self {
            index: 0,
            items: vec![vec![]; BUFFER_SIZE],
            map: HashMap::new(),
        }
    }

    pub fn insert_packet(&mut self, packet: Vec<u8>) {
        if self.index == BUFFER_SIZE {
            self.index = 0;
        }

        let curr_seq = Bytes::copy_from_slice(&packet[2..4]).get_u16();

        if let Some(prev_packet_seq) = self
            .map
            .get(&curr_seq)
            .and_then(|index| self.items.get(*index))
            .filter(|packet| !packet.is_empty())
            .map(|packet| Bytes::copy_from_slice(&packet[2..4]).get_u16())
        {
            self.map.remove(&prev_packet_seq);
        }

        self.items[self.index] = packet;
        self.map.insert(curr_seq, self.index);
        self.index = self.index + 1;
    }

    pub fn get_packet(&self, seq: u16) -> Option<&Vec<u8>> {
        self.map.get(&seq).and_then(|index| self.items.get(*index))
    }
}
