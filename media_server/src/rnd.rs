use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;

pub fn get_random_string(size: usize) -> String {
    thread_rng().sample_iter(Alphanumeric).take(size).map(char::from).collect()
}