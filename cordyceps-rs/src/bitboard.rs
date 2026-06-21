//! 3 x u64 bitboard covering 170 cells (170/192 bits used).

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bitboard {
    pub lo: u64,  // cells 0-63
    pub mid: u64, // cells 64-127
    pub hi: u64,  // cells 128-169 (42 bits used)
}

impl Bitboard {
    #[inline]
    pub const fn empty() -> Self {
        Bitboard { lo: 0, mid: 0, hi: 0 }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn popcount(&self) -> i32 {
        (self.lo.count_ones() + self.mid.count_ones() + self.hi.count_ones()) as i32
    }

    #[inline]
    pub fn set(&mut self, idx: usize) {
        if idx < 64 {
            self.lo |= 1u64 << idx;
        } else if idx < 128 {
            self.mid |= 1u64 << (idx - 64);
        } else {
            self.hi |= 1u64 << (idx - 128);
        }
    }

    #[inline]
    pub fn clear(&mut self, idx: usize) {
        if idx < 64 {
            self.lo &= !(1u64 << idx);
        } else if idx < 128 {
            self.mid &= !(1u64 << (idx - 64));
        } else {
            self.hi &= !(1u64 << (idx - 128));
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn test(&self, idx: usize) -> bool {
        if idx < 64 {
            (self.lo >> idx) & 1 != 0
        } else if idx < 128 {
            (self.mid >> (idx - 64)) & 1 != 0
        } else {
            (self.hi >> (idx - 128)) & 1 != 0
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        (self.lo | self.mid | self.hi) == 0
    }
}

impl BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, rhs: Self) {
        self.lo &= rhs.lo;
        self.mid &= rhs.mid;
        self.hi &= rhs.hi;
    }
}
impl BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, rhs: Self) {
        self.lo |= rhs.lo;
        self.mid |= rhs.mid;
        self.hi |= rhs.hi;
    }
}
impl BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.lo ^= rhs.lo;
        self.mid ^= rhs.mid;
        self.hi ^= rhs.hi;
    }
}
impl BitAnd for Bitboard {
    type Output = Bitboard;
    fn bitand(mut self, rhs: Self) -> Bitboard {
        self &= rhs;
        self
    }
}
impl BitOr for Bitboard {
    type Output = Bitboard;
    fn bitor(mut self, rhs: Self) -> Bitboard {
        self |= rhs;
        self
    }
}
impl BitXor for Bitboard {
    type Output = Bitboard;
    fn bitxor(mut self, rhs: Self) -> Bitboard {
        self ^= rhs;
        self
    }
}
