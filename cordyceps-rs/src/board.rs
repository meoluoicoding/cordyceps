//! Board state, move application/undo, and the static evaluation function.

use std::cell::Cell;

use crate::bitboard::Bitboard;
use crate::consts::{K_CELLS, K_COLS, K_NO_OWNER, K_PLAYER_OPP, K_PLAYER_US, K_ROWS};
use crate::types::Move;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvalCache {
    pub my_territory: i32,
    pub opp_territory: i32,
    pub my_corners: i32,
    pub opp_corners: i32,
    pub my_edges: i32,
    pub opp_edges: i32,
    pub live_adj_my: i32,
    pub live_adj_opp: i32,
    pub connectivity_my: i32,
    pub connectivity_opp: i32,
}

/// Undo information for `Board::unmake_move`. `changed` holds (cell_index, old_value)
/// pairs for every mushroom cleared by the move (empty for a pass move).
pub struct UndoMove {
    pub mv: Move,
    pub changed: Vec<(usize, i8)>,
    pub old_my_mask: Bitboard,
    pub old_opp_mask: Bitboard,
    pub old_live_mask: Bitboard,
    pub old_live_count: i32,
    pub old_my_score: i32,
    pub old_opp_score: i32,
    pub old_consecutive_passes: i32,
    pub old_current_player: i32,
    pub old_eval_cache: EvalCache,
}

#[derive(Clone)]
pub struct Board {
    pub values: Vec<i8>, // 0=empty, 1-9=mushroom, len K_CELLS
    pub owners: Vec<i8>, // 0=none, 1=us, -1=opp, len K_CELLS

    pub my_mask: Bitboard,
    pub opp_mask: Bitboard,
    pub live_mask: Bitboard,

    pub my_score: i32,
    pub opp_score: i32,
    pub live_count: i32,
    pub current_player: i32, // 1 = us, -1 = opp
    pub consecutive_passes: i32,

    pub eval_cache: EvalCache,
}

impl Default for Board {
    fn default() -> Self {
        Board::new()
    }
}

impl Board {
    pub fn new() -> Self {
        Board {
            values: vec![0i8; K_CELLS],
            owners: vec![0i8; K_CELLS],
            my_mask: Bitboard::empty(),
            opp_mask: Bitboard::empty(),
            live_mask: Bitboard::empty(),
            my_score: 0,
            opp_score: 0,
            live_count: 0,
            current_player: 0,
            consecutive_passes: 0,
            eval_cache: EvalCache::default(),
        }
    }

    /// Bounds-checked cell value read. Out-of-range coordinates fall back to
    /// `values[0]`, mirroring the safety fallback in the original C++.
    #[inline]
    pub fn value_at(&self, r: i32, c: i32) -> i8 {
        if r < 0 || r >= K_ROWS as i32 || c < 0 || c >= K_COLS as i32 {
            self.values[0]
        } else {
            self.values[(r * K_COLS as i32 + c) as usize]
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn owner_at(&self, r: i32, c: i32) -> i8 {
        if r < 0 || r >= K_ROWS as i32 || c < 0 || c >= K_COLS as i32 {
            self.owners[0]
        } else {
            self.owners[(r * K_COLS as i32 + c) as usize]
        }
    }

    pub fn recalc_live_mask(&mut self) {
        self.live_mask = Bitboard::empty();
        self.live_count = 0;
        for i in 0..K_CELLS {
            if self.values[i] > 0 {
                self.live_mask.set(i);
                self.live_count += 1;
            }
        }
    }

    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.consecutive_passes >= 2
    }

    #[inline]
    pub fn score_from_perspective(&self, player: i32) -> i32 {
        if player == K_PLAYER_US {
            self.my_score - self.opp_score
        } else {
            self.opp_score - self.my_score
        }
    }

    pub fn apply_move(&mut self, mv: Move) -> UndoMove {
        let old_my_mask = self.my_mask;
        let old_opp_mask = self.opp_mask;
        let old_live_mask = self.live_mask;
        let old_live_count = self.live_count;
        let old_my_score = self.my_score;
        let old_opp_score = self.opp_score;
        let old_consecutive_passes = self.consecutive_passes;
        let old_current_player = self.current_player;
        let old_eval_cache = self.eval_cache;

        if mv.is_pass() {
            self.consecutive_passes += 1;
            self.current_player = -self.current_player;
            return UndoMove {
                mv,
                changed: Vec::new(),
                old_my_mask,
                old_opp_mask,
                old_live_mask,
                old_live_count,
                old_my_score,
                old_opp_score,
                old_consecutive_passes,
                old_current_player,
                old_eval_cache,
            };
        }

        self.consecutive_passes = 0;
        let player = self.current_player;

        let (r1, c1, r2, c2) = (mv.r1 as i32, mv.c1 as i32, mv.r2 as i32, mv.c2 as i32);
        let mut changed: Vec<(usize, i8)> = Vec::new();

        const DR: [i32; 4] = [-1, 1, 0, 0];
        const DC: [i32; 4] = [0, 0, -1, 1];

        // Pass 1: clear mushrooms in the rectangle, recording old values, and
        // decrement stale adjacent-live counters.
        for r in r1..=r2 {
            for c in c1..=c2 {
                let idx_i = r * K_COLS as i32 + c;
                if idx_i < 0 || idx_i >= K_CELLS as i32 {
                    continue; // safety, mirrors the C++ bounds guard
                }
                let idx = idx_i as usize;
                if self.values[idx] > 0 {
                    changed.push((idx, self.values[idx]));

                    for d in 0..4 {
                        let nr = r + DR[d];
                        let nc = c + DC[d];
                        if nr >= 0 && nr < K_ROWS as i32 && nc >= 0 && nc < K_COLS as i32 {
                            let nidx = (nr * K_COLS as i32 + nc) as usize;
                            if self.owners[nidx] as i32 == K_PLAYER_US {
                                if self.eval_cache.live_adj_my > 0 {
                                    self.eval_cache.live_adj_my -= 1;
                                }
                            } else if self.owners[nidx] as i32 == K_PLAYER_OPP {
                                if self.eval_cache.live_adj_opp > 0 {
                                    self.eval_cache.live_adj_opp -= 1;
                                }
                            }
                        }
                    }

                    self.values[idx] = 0;
                    self.live_mask.clear(idx);
                    self.live_count -= 1;
                }
            }
        }

        // Pass 2: assign ownership and update eval-cache stats.
        for &(idx, old_val) in &changed {
            let r = (idx / K_COLS) as i32;
            let c = (idx % K_COLS) as i32;

            self.owners[idx] = player as i8;

            for d in 0..4 {
                let nr2 = r + DR[d];
                let nc2 = c + DC[d];
                if nr2 >= 0 && nr2 < K_ROWS as i32 && nc2 >= 0 && nc2 < K_COLS as i32 {
                    let nidx2 = (nr2 * K_COLS as i32 + nc2) as usize;
                    if self.owners[nidx2] as i32 == player {
                        if player == K_PLAYER_US {
                            self.eval_cache.connectivity_my += 1;
                        } else {
                            self.eval_cache.connectivity_opp += 1;
                        }
                    }
                }
            }

            if player == K_PLAYER_US {
                self.my_mask.set(idx);
                self.eval_cache.my_territory += 1;
                if is_corner_cell(r, c) {
                    self.eval_cache.my_corners += 1;
                }
                if is_edge_cell(r, c) {
                    self.eval_cache.my_edges += 1;
                }
                self.my_score += old_val as i32;

                for d in 0..4 {
                    let nr = r + DR[d];
                    let nc = c + DC[d];
                    if nr >= 0 && nr < K_ROWS as i32 && nc >= 0 && nc < K_COLS as i32 {
                        let nidx = (nr * K_COLS as i32 + nc) as usize;
                        if self.values[nidx] > 0 && self.owners[nidx] as i32 == K_NO_OWNER {
                            self.eval_cache.live_adj_my += 1;
                        }
                    }
                }
            } else {
                self.opp_mask.set(idx);
                self.eval_cache.opp_territory += 1;
                if is_corner_cell(r, c) {
                    self.eval_cache.opp_corners += 1;
                }
                if is_edge_cell(r, c) {
                    self.eval_cache.opp_edges += 1;
                }
                self.opp_score += old_val as i32;

                for d in 0..4 {
                    let nr = r + DR[d];
                    let nc = c + DC[d];
                    if nr >= 0 && nr < K_ROWS as i32 && nc >= 0 && nc < K_COLS as i32 {
                        let nidx = (nr * K_COLS as i32 + nc) as usize;
                        if self.values[nidx] > 0 && self.owners[nidx] as i32 == K_NO_OWNER {
                            self.eval_cache.live_adj_opp += 1;
                        }
                    }
                }
            }
        }

        self.current_player = -self.current_player;

        UndoMove {
            mv,
            changed,
            old_my_mask,
            old_opp_mask,
            old_live_mask,
            old_live_count,
            old_my_score,
            old_opp_score,
            old_consecutive_passes,
            old_current_player,
            old_eval_cache,
        }
    }

    pub fn barrier_potential(&self, r1: i32, c1: i32, r2: i32, c2: i32) -> i32 {
        let h = r2 - r1 + 1;
        let w = c2 - c1 + 1;
        if h >= 4 && w <= 2 {
            let cc = (c1 + c2) / 2;
            if cc >= 4 && cc <= 12 { return 2; }
            return 1;
        }
        if w >= 6 && h <= 2 {
            let cr = (r1 + r2) / 2;
            if cr >= 3 && cr <= 6 { return 2; }
            return 1;
        }
        0
    }

    pub fn dead_cell_risk_proxy(&self, r1: i32, c1: i32, r2: i32, c2: i32) -> i32 {
        let mut risk = 0i32;
        for r in r1..=r2 {
            for c in c1..=c2 {
                let idx = (r as usize) * K_COLS + (c as usize);
                if self.owners[idx] as i32 == self.current_player { continue; }
                let mut protection = 0i32;
                if r > 0 && self.owners[((r - 1) as usize) * K_COLS + c as usize] as i32 == self.current_player { protection += 1; }
                if r < K_ROWS as i32 - 1 && self.owners[((r + 1) as usize) * K_COLS + c as usize] as i32 == self.current_player { protection += 1; }
                if c > 0 && self.owners[r as usize * K_COLS + (c - 1) as usize] as i32 == self.current_player { protection += 1; }
                if c < K_COLS as i32 - 1 && self.owners[r as usize * K_COLS + (c + 1) as usize] as i32 == self.current_player { protection += 1; }
                let adjacent_live = (r > 0 && self.values[((r - 1) as usize) * K_COLS + c as usize] > 0)
                    || (r < K_ROWS as i32 - 1 && self.values[((r + 1) as usize) * K_COLS + c as usize] > 0)
                    || (c > 0 && self.values[r as usize * K_COLS + (c - 1) as usize] > 0)
                    || (c < K_COLS as i32 - 1 && self.values[r as usize * K_COLS + (c + 1) as usize] > 0);
                if protection == 0 && adjacent_live { risk += 2; }
                else if protection <= 1 && adjacent_live { risk += 1; }
            }
        }
        risk
    }

    pub fn connectivity_boost(&self, r1: i32, c1: i32, r2: i32, c2: i32) -> i32 {
        let mut boost = 0i32;
        for r in r1..=r2 {
            for c in c1..=c2 {
                let idx = (r as usize) * K_COLS + (c as usize);
                if self.owners[idx] as i32 == self.current_player { continue; }
                if r > 0 && self.owners[((r - 1) as usize) * K_COLS + c as usize] as i32 == self.current_player { boost += 1; }
                if r < K_ROWS as i32 - 1 && self.owners[((r + 1) as usize) * K_COLS + c as usize] as i32 == self.current_player { boost += 1; }
                if c > 0 && self.owners[r as usize * K_COLS + (c - 1) as usize] as i32 == self.current_player { boost += 1; }
                if c < K_COLS as i32 - 1 && self.owners[r as usize * K_COLS + (c + 1) as usize] as i32 == self.current_player { boost += 1; }
            }
        }
        boost
    }

    pub fn unmake_move(&mut self, undo: &UndoMove) {
        if !undo.mv.is_pass() {
            for &(idx, old_val) in &undo.changed {
                self.values[idx] = old_val;
                self.owners[idx] = K_NO_OWNER as i8;
            }
        }

        self.my_mask = undo.old_my_mask;
        self.opp_mask = undo.old_opp_mask;
        self.live_mask = undo.old_live_mask;
        self.live_count = undo.old_live_count;
        self.my_score = undo.old_my_score;
        self.opp_score = undo.old_opp_score;
        self.consecutive_passes = undo.old_consecutive_passes;
        self.current_player = undo.old_current_player;
        self.eval_cache = undo.old_eval_cache;
    }
}

#[inline]
fn is_corner_cell(r: i32, c: i32) -> bool {
    (r == 0 || r == K_ROWS as i32 - 1) && (c == 0 || c == K_COLS as i32 - 1)
}

#[inline]
fn is_edge_cell(r: i32, c: i32) -> bool {
    r == 0 || r == K_ROWS as i32 - 1 || c == 0 || c == K_COLS as i32 - 1
}

// ===== Runtime weight loading for tuning =====
// Thread-local: zero overhead when not in tune mode. Order: score, territory,
// corners, edges, live_adj, recapture, vulnerability (the last two are accepted
// for API compatibility but unused by `evaluate`, matching the original).
thread_local! {
    static TUNE_W: Cell<[i32; 7]> = Cell::new([0; 7]);
    static TUNE_ACTIVE: Cell<bool> = Cell::new(false);
}

#[allow(dead_code)]
pub fn set_tune_weights(
    score_w: i32,
    territory_w: i32,
    corner_w: i32,
    edge_w: i32,
    adj_w: i32,
    recapture_w: i32,
    vulnerability_w: i32,
) {
    TUNE_W.with(|w| w.set([score_w, territory_w, corner_w, edge_w, adj_w, recapture_w, vulnerability_w]));
    TUNE_ACTIVE.with(|a| a.set(true));
}

#[allow(dead_code)]
pub fn clear_tune_weights() {
    TUNE_ACTIVE.with(|a| a.set(false));
}

pub fn evaluate(board: &Board, player: i32) -> i32 {
    let ec = &board.eval_cache;

    let mut territory_diff = ec.my_territory - ec.opp_territory;
    let mut corner_diff = ec.my_corners - ec.opp_corners;
    let mut edge_diff = ec.my_edges - ec.opp_edges;
    let mut conn_diff = ec.connectivity_my - ec.connectivity_opp;

    // recapture_swing = opponent's live_adjacent (bonus: cells we can recapture)
    // vulnerability = our live_adjacent (penalty: cells opponent can recapture)
    let (recapture_swing, vulnerability) = if player == K_PLAYER_US {
        (ec.live_adj_opp, ec.live_adj_my)
    } else {
        (ec.live_adj_my, ec.live_adj_opp)
    };

    if player == K_PLAYER_OPP {
        territory_diff = -territory_diff;
        corner_diff = -corner_diff;
        edge_diff = -edge_diff;
        conn_diff = -conn_diff;
    }

    let tune_active = TUNE_ACTIVE.with(|a| a.get());
    if tune_active {
        let w = TUNE_W.with(|w| w.get());
        return territory_diff * w[1]
            + corner_diff * w[2]
            + edge_diff * w[3]
            + recapture_swing * w[4]
            - vulnerability * w[5]
            + conn_diff * 0;
    }

    // mushroom-bot side-specific weights
    let (territory_w, connectivity_w, corner_w, edge_w, recapture_w, vulnerability_w) =
        if player == K_PLAYER_US {
            (148, 19, 18, 3, 39, 9)   // FIRST weights
        } else {
            (140, 28, 20, 6, 28, 18)  // SECOND weights
        };

    territory_diff * territory_w
        + conn_diff * connectivity_w
        + corner_diff * corner_w
        + edge_diff * edge_w
        + recapture_swing * recapture_w
        - vulnerability * vulnerability_w
}
