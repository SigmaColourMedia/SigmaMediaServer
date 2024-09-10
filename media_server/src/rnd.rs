use rand::{RngCore, thread_rng};

pub fn get_random_id() -> u32 {
    thread_rng().next_u32()
}
