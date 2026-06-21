//! Zobrist hashing for transposition-table keys.
//!
//! Note: the original C++ seeds `std::mt19937_64` with a fixed constant. Since
//! these random values never leave the process (no cross-run/cross-language
//! persistence), we use a small deterministic SplitMix64 generator seeded the
//! same way instead of reimplementing MT19937-64 bit-for-bit. This preserves
//! the important property (a fixed, reproducible hash table per run) without
//! pulling in an external RNG crate.

use crate::board::Board;
use crate::consts::{K_CELLS, K_PLAYER_OPP, K_PLAYER_US};

const SEED: u64 = 123_456_789;

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[derive(Clone)]
pub struct Zobrist {
    z_value: Vec<[u64; 10]>, // [cell_idx][value 1-9] (index 0 unused, matches original)
    z_owner: Vec<[u64; 2]>,  // [cell_idx][0=US, 1=OPP]
    z_player: [u64; 2],      // [0=US, 1=OPP]
    z_passes: Vec<u64>,      // [consecutive_passes]
}

impl Zobrist {
    pub fn new() -> Self {
        let mut rng = SplitMix64::new(SEED);

        let mut z_value = Vec::with_capacity(K_CELLS);
        let mut z_owner = Vec::with_capacity(K_CELLS);
        for _ in 0..K_CELLS {
            let mut vals = [0u64; 10];
            for v in vals.iter_mut() {
                *v = rng.next_u64();
            }
            z_value.push(vals);
            z_owner.push([rng.next_u64(), rng.next_u64()]);
        }

        let z_player = [rng.next_u64(), rng.next_u64()];

        let mut z_passes = Vec::with_capacity(K_CELLS);
        for _ in 0..K_CELLS {
            z_passes.push(rng.next_u64());
        }

        Zobrist { z_value, z_owner, z_player, z_passes }
    }

    pub fn compute(&self, board: &Board) -> u64 {
        let mut h: u64 = 0;

        for i in 0..K_CELLS {
            let v = board.values[i];
            let o = board.owners[i];

            if v > 0 {
                h ^= self.z_value[i][v as usize];
            }
            if o as i32 == K_PLAYER_US {
                h ^= self.z_owner[i][0];
            } else if o as i32 == K_PLAYER_OPP {
                h ^= self.z_owner[i][1];
            }
        }

        if board.current_player == K_PLAYER_US {
            h ^= self.z_player[0];
        } else if board.current_player == K_PLAYER_OPP {
            h ^= self.z_player[1];
        }

        h ^= self.z_passes[board.consecutive_passes as usize];
        h
    }
}
