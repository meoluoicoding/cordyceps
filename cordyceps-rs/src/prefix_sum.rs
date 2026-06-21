//! 2D prefix sum for O(1) rectangle-sum queries over the board.

use crate::board::Board;
use crate::consts::{K_COLS, K_ROWS};

pub struct PrefixSum {
    raw: Vec<i8>,   // K_ROWS x K_COLS
    pref: Vec<i32>, // (K_ROWS+1) x (K_COLS+1), sentinel row/col
}

impl PrefixSum {
    pub fn new() -> Self {
        PrefixSum {
            raw: vec![0i8; K_ROWS * K_COLS],
            pref: vec![0i32; (K_ROWS + 1) * (K_COLS + 1)],
        }
    }

    #[inline]
    pub fn set(&mut self, r: usize, c: usize, val: i8) {
        self.raw[r * K_COLS + c] = val;
    }

    pub fn build(&mut self) {
        for r in 0..=K_ROWS {
            for c in 0..=K_COLS {
                let idx = r * (K_COLS + 1) + c;
                if r == 0 || c == 0 {
                    self.pref[idx] = 0;
                } else {
                    let up = self.pref[(r - 1) * (K_COLS + 1) + c];
                    let left = self.pref[r * (K_COLS + 1) + (c - 1)];
                    let diag = self.pref[(r - 1) * (K_COLS + 1) + (c - 1)];
                    let raw_v = self.raw[(r - 1) * K_COLS + (c - 1)] as i32;
                    self.pref[idx] = up + left - diag + raw_v;
                }
            }
        }
    }

    /// O(1) sum of rectangle (r1,c1)-(r2,c2) inclusive.
    #[inline]
    pub fn sum(&self, r1: i32, c1: i32, r2: i32, c2: i32) -> i32 {
        let rr1 = r1 as usize;
        let cc1 = c1 as usize;
        let rr2 = (r2 + 1) as usize;
        let cc2 = (c2 + 1) as usize;
        self.pref[rr2 * (K_COLS + 1) + cc2] - self.pref[rr1 * (K_COLS + 1) + cc2]
            - self.pref[rr2 * (K_COLS + 1) + cc1]
            + self.pref[rr1 * (K_COLS + 1) + cc1]
    }

    pub fn from_board(board: &Board) -> Self {
        let mut ps = PrefixSum::new();
        for r in 0..K_ROWS {
            for c in 0..K_COLS {
                ps.set(r, c, board.value_at(r as i32, c as i32));
            }
        }
        ps.build();
        ps
    }
}
