//! Legal move (sum-to-10, inscribed rectangle) generation.

use crate::board::Board;
use crate::consts::{K_COLS, K_ROWS, K_TARGET_SUM};
use crate::prefix_sum::PrefixSum;
use crate::rect_table::RectTable;
use crate::types::Move;

/// Direct-scan rectangle sum (used for reference/testing; the optimized path
/// uses `PrefixSum` instead).
#[allow(dead_code)]
pub fn rect_sum(board: &Board, r1: i32, c1: i32, r2: i32, c2: i32) -> i32 {
    let mut sum = 0;
    for r in r1..=r2 {
        for c in c1..=c2 {
            sum += board.value_at(r, c) as i32;
        }
    }
    sum
}

/// Inscribed rule: all 4 edges of the rectangle must touch at least one
/// remaining mushroom.
pub fn check_inscribed(board: &Board, r1: i32, c1: i32, r2: i32, c2: i32) -> bool {
    let mut ok = false;
    for c in c1..=c2 {
        if board.value_at(r1, c) > 0 {
            ok = true;
            break;
        }
    }
    if !ok {
        return false;
    }

    ok = false;
    for c in c1..=c2 {
        if board.value_at(r2, c) > 0 {
            ok = true;
            break;
        }
    }
    if !ok {
        return false;
    }

    ok = false;
    for r in r1..=r2 {
        if board.value_at(r, c1) > 0 {
            ok = true;
            break;
        }
    }
    if !ok {
        return false;
    }

    ok = false;
    for r in r1..=r2 {
        if board.value_at(r, c2) > 0 {
            ok = true;
            break;
        }
    }
    ok
}

/// Brute-force legal move generation (sum=10, inscribed). Kept for parity with
/// the original; `generate_legal_moves_optimized` is used in the hot path.
#[allow(dead_code)]
pub fn generate_legal_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(256);

    for r1 in 0..K_ROWS as i32 {
        for r2 in r1..K_ROWS as i32 {
            let mut col_sums = vec![0i32; K_COLS];
            for c in 0..K_COLS {
                for r in r1..=r2 {
                    col_sums[c] += board.value_at(r, c as i32) as i32;
                }
            }

            for c1 in 0..K_COLS as i32 {
                let mut wsum = 0;
                for c2 in c1..K_COLS as i32 {
                    wsum += col_sums[c2 as usize];
                    if wsum > K_TARGET_SUM {
                        break;
                    }

                    if wsum == K_TARGET_SUM && check_inscribed(board, r1, c1, r2, c2) {
                        moves.push(Move {
                            r1: r1 as i8,
                            c1: c1 as i8,
                            r2: r2 as i8,
                            c2: c2 as i8,
                            score_hint: 0,
                        });
                    }
                }
            }
        }
    }

    moves
}

/// Optimized legal move generation using the precomputed `RectTable` plus an
/// O(1) prefix-sum rectangle query.
pub fn generate_legal_moves_optimized(board: &Board, table: &RectTable) -> Vec<Move> {
    let ps = PrefixSum::from_board(board);
    let mut moves = Vec::with_capacity(256);

    let n = table.num_rects();
    for i in 0..n {
        let ri = table.get_rect(i);

        let r1 = ri.r1 as i32;
        let c1 = ri.c1 as i32;
        let r2 = ri.r2 as i32;
        let c2 = ri.c2 as i32;

        if r1 < 0 || r2 >= K_ROWS as i32 || c1 < 0 || c2 >= K_COLS as i32 {
            continue;
        }

        if ps.sum(r1, c1, r2, c2) != K_TARGET_SUM {
            continue;
        }

        let live = board.live_mask;
        if (ri.top_mask & live).is_empty() {
            continue;
        }
        if (ri.bottom_mask & live).is_empty() {
            continue;
        }
        if (ri.left_mask & live).is_empty() {
            continue;
        }
        if (ri.right_mask & live).is_empty() {
            continue;
        }

        moves.push(Move { r1: ri.r1, c1: ri.c1, r2: ri.r2, c2: ri.c2, score_hint: 0 });
    }

    moves
}
