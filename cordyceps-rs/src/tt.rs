//! Transposition table: depth-preferred replacement, always-replace on same key.

use crate::types::{Move, PASS_MOVE};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TTFlag {
    Empty,
    Exact,
    Alpha,
    Beta,
}

#[derive(Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub best_move: Move,
    pub score: i32,
    pub depth: i8,
    pub flag: TTFlag,
}

impl Default for TTEntry {
    fn default() -> Self {
        TTEntry { key: 0, best_move: PASS_MOVE, score: 0, depth: 0, flag: TTFlag::Empty }
    }
}

pub struct TranspositionTable {
    table: Vec<TTEntry>,
    mask: u64,
}

impl TranspositionTable {
    pub fn new(size_power: u32) -> Self {
        let sz = 1usize << size_power;
        TranspositionTable { table: vec![TTEntry::default(); sz], mask: (sz - 1) as u64 }
    }

    /// Probe the table. If found and valid for `depth`, fills `score`/`mv` and
    /// returns the stored flag; otherwise returns `TTFlag::Empty` and leaves
    /// `score`/`mv` untouched by the caller's perspective (matches the C++
    /// out-parameter style of the original).
    pub fn probe(&self, key: u64, depth: i32, score: &mut i32, mv: &mut Move) -> TTFlag {
        let idx = (key & self.mask) as usize;
        let entry = &self.table[idx];

        if entry.key != key {
            return TTFlag::Empty;
        }
        if (entry.depth as i32) < depth {
            return TTFlag::Empty;
        }

        *score = entry.score;
        *mv = entry.best_move;
        entry.flag
    }

    pub fn store(&mut self, key: u64, depth: i32, flag: TTFlag, score: i32, mv: Move) {
        let idx = (key & self.mask) as usize;
        let entry = &mut self.table[idx];

        // Depth-preferred: keep a deeper entry from a different position.
        // Always replace the same position.
        if entry.flag != TTFlag::Empty && entry.key != key && (entry.depth as i32) > depth {
            return;
        }

        entry.key = key;
        entry.depth = depth as i8;
        entry.flag = flag;
        entry.score = score;
        entry.best_move = mv;
    }

    pub fn clear(&mut self) {
        for e in self.table.iter_mut() {
            e.key = 0;
            e.flag = TTFlag::Empty;
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.table.len()
    }
}
