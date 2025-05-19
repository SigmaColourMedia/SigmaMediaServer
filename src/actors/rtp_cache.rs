use bytes::{Buf, Bytes};
use cached::{Cached, TimedSizedCache};

pub struct RTPCache {
    cache: TimedSizedCache<u16, Vec<u8>>,
}

impl RTPCache {
    pub fn new() -> Self {
        Self {
            cache: TimedSizedCache::with_size_and_lifespan(400, 6),
        }
    }
    pub fn insert_packet(&mut self, packet: Vec<u8>) {
        let seq = Bytes::copy_from_slice(&packet).slice(2..).get_u16();
        self.cache.cache_set(seq, packet);
    }

    pub fn get_packet(&mut self, seq: u16) -> Option<&Vec<u8>> {
        self.cache.cache_get(&seq)
    }
}
