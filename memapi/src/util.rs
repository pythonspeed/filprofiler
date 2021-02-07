use ahash::RandomState as ARandomState;
use std::collections::HashMap;

lazy_static! {
    // If the PYTHONHASHSEED environment variable is set, we will use it as seed
    // for Rust hashmaps as well, to reduce randomness when benchmarking.
    static ref HASH_SEED: Option<u64> = match std::env::var("PYTHONHASHSEED") {
        Ok(value) => {
            if value == "random" {
                None
            } else {
                let seed = value.parse::<i64>().unwrap();
                Some(seed as u64)
            }
        }
        _ => None,
    };
}

/// Create a new hashmap with an optional fixed seed.
pub fn new_hashmap<K, V>() -> HashMap<K, V, ARandomState> {
    match *HASH_SEED {
        Some(seed) => {
            HashMap::with_hasher(ARandomState::with_seeds(seed, seed + 1, seed + 2, seed + 3))
        }
        None => HashMap::default(),
    }
}
