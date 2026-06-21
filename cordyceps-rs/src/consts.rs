//! Core board/game constants, mirroring the original C++ `cordyceps` namespace.

pub const K_ROWS: usize = 10;
pub const K_COLS: usize = 17;
pub const K_CELLS: usize = K_ROWS * K_COLS; // 170
pub const K_TARGET_SUM: i32 = 10;
#[allow(dead_code)]
pub const K_NUM_RECTS: usize = 8415; // all possible rects on a 10x17 grid

pub const K_PLAYER_US: i32 = 1;
pub const K_PLAYER_OPP: i32 = -1;
pub const K_NO_OWNER: i32 = 0;
