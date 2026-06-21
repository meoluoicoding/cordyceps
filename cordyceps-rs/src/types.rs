//! Small shared value types: `Move`, `SideConfig`, `GamePhase`.

/// A candidate/played move: a rectangle (r1,c1)-(r2,c2) inclusive.
/// `r1 == -1` is the sentinel "pass" move (mirrors `k_pass_move` in the C++ original).
#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub r1: i8,
    pub c1: i8,
    pub r2: i8,
    pub c2: i8,
    /// Temporary sort key set by `enhance_root_moves`. Not part of equality.
    pub score_hint: i32,
}

impl Move {
    #[inline]
    pub fn is_pass(&self) -> bool {
        self.r1 == -1
    }
}

// Equality intentionally ignores `score_hint`, matching the C++ operator==.
impl PartialEq for Move {
    fn eq(&self, other: &Self) -> bool {
        self.r1 == other.r1 && self.c1 == other.c1 && self.r2 == other.r2 && self.c2 == other.c2
    }
}
impl Eq for Move {}

/// The pass move sentinel (matches `k_pass_move`).
pub const PASS_MOVE: Move = Move { r1: -1, c1: -1, r2: -1, c2: -1, score_hint: 0 };

/// The "zero" / default-constructed move (matches a value-initialized `Move{}` in C++,
/// used to reset killer-move slots). NOT the same as `PASS_MOVE`.
pub const ZERO_MOVE: Move = Move { r1: 0, c1: 0, r2: 0, c2: 0, score_hint: 0 };

#[derive(Clone, Copy, Debug, Default)]
pub struct SideConfig {
    pub time_multiplier: f32,
    pub aggression: f32,
    pub steal_bonus: f32,
    pub defense_bonus: f32,
    pub prefer_vertical: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    Opening,
    Midgame,
    Late,
    Endgame,
}
