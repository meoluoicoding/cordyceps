// ===== src/consts.rs =====


pub const K_ROWS: usize = 10;
pub const K_COLS: usize = 17;
pub const K_CELLS: usize = K_ROWS * K_COLS; // 170
pub const K_TARGET_SUM: i32 = 10;
#[allow(dead_code)]
pub const K_NUM_RECTS: usize = 8415; // all possible rects on a 10x17 grid

pub const K_PLAYER_US: i32 = 1;
pub const K_PLAYER_OPP: i32 = -1;
pub const K_NO_OWNER: i32 = 0;



// ===== src/types.rs =====


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



// ===== src/bitboard.rs =====


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



// ===== src/prefix_sum.rs =====



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



// ===== src/board.rs =====


use std::cell::Cell;


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
    let score = board.score_from_perspective(player);

    let mut territory_diff = ec.my_territory - ec.opp_territory;
    let mut corner_diff = ec.my_corners - ec.opp_corners;
    let mut edge_diff = ec.my_edges - ec.opp_edges;
    let mut adj_diff = ec.live_adj_my - ec.live_adj_opp;
    let mut conn_diff = ec.connectivity_my - ec.connectivity_opp;

    if player == K_PLAYER_OPP {
        territory_diff = -territory_diff;
        corner_diff = -corner_diff;
        edge_diff = -edge_diff;
        adj_diff = -adj_diff;
        conn_diff = -conn_diff;
    }

    let tune_active = TUNE_ACTIVE.with(|a| a.get());
    if tune_active {
        let w = TUNE_W.with(|w| w.get());
        return score * w[0]
            + territory_diff * w[1]
            + corner_diff * w[2]
            + edge_diff * w[3]
            + adj_diff * w[4]
            + conn_diff * 0;
    }

    // Baseline weights (proven: 38% vs agent+superchym, FIRST 63%)
    // Score *3, Territory *3, Corners *8, Edges *2, LiveAdj *3
    score * 3 + territory_diff * 3 + corner_diff * 8 + edge_diff * 2 + adj_diff * 3 + conn_diff * 0
}



// ===== src/rect_table.rs =====


use std::convert::TryInto;
use std::fs::File;
use std::io::Read;


const MAGIC: u32 = 0x4352_4459;
const PACKED_RECT_SIZE: usize = 106;

#[derive(Clone, Copy)]
pub struct RectInfo {
    pub r1: i8,
    pub c1: i8,
    pub r2: i8,
    pub c2: i8,
    pub cell_count: u16,
    pub cell_offset: u32,
    pub top_mask: Bitboard,
    pub bottom_mask: Bitboard,
    pub left_mask: Bitboard,
    pub right_mask: Bitboard,
}

#[derive(Clone)]
pub struct RectTable {
    rects: Vec<RectInfo>,
    cell_table: Vec<u8>,
}

impl RectTable {
    /// Loads the table from disk. Returns `None` on any I/O or format error,
    /// mirroring the C++ `load()` returning `false`.
    pub fn load(filename: &str) -> Option<Self> {
        let mut f = File::open(filename).ok()?;

        let mut header = [0u8; 16];
        f.read_exact(&mut header).ok()?;

        let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
        if magic != MAGIC {
            return None;
        }
        let num_rects = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let cell_table_size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        // header[12..16] is the checksum; read but not validated (matches original).

        let mut rects = Vec::with_capacity(num_rects);
        let mut buf = [0u8; PACKED_RECT_SIZE];

        for _ in 0..num_rects {
            f.read_exact(&mut buf).ok()?;

            let r1 = buf[0] as i8;
            let c1 = buf[1] as i8;
            let r2 = buf[2] as i8;
            let c2 = buf[3] as i8;
            let cell_count = u16::from_le_bytes(buf[4..6].try_into().unwrap());
            let cell_offset = u32::from_le_bytes(buf[6..10].try_into().unwrap());

            let read_u64 = |off: usize| -> u64 { u64::from_le_bytes(buf[off..off + 8].try_into().unwrap()) };

            let top_mask = Bitboard { lo: read_u64(10), mid: read_u64(18), hi: read_u64(26) };
            let bottom_mask = Bitboard { lo: read_u64(34), mid: read_u64(42), hi: read_u64(50) };
            let left_mask = Bitboard { lo: read_u64(58), mid: read_u64(66), hi: read_u64(74) };
            let right_mask = Bitboard { lo: read_u64(82), mid: read_u64(90), hi: read_u64(98) };

            rects.push(RectInfo {
                r1,
                c1,
                r2,
                c2,
                cell_count,
                cell_offset,
                top_mask,
                bottom_mask,
                left_mask,
                right_mask,
            });
        }

        let mut cell_table = vec![0u8; cell_table_size];
        f.read_exact(&mut cell_table).ok()?;

        Some(RectTable { rects, cell_table })
    }

    #[inline]
    #[allow(dead_code)]
    pub fn num_rects(&self) -> usize {
        self.rects.len()
    }

    /// O(1) rectangle id from coordinates, matching the original closed-form formula.
    #[inline]
    pub fn rect_id(&self, r1: i32, c1: i32, r2: i32, c2: i32) -> usize {
        const K_NUM_COL_PAIRS: i32 = (K_COLS as i32) * (K_COLS as i32 + 1) / 2; // 153

        let row_offset = r1 * K_ROWS as i32 - r1 * (r1 - 1) / 2;
        let row_pair = row_offset + (r2 - r1);

        let col_offset = c1 * K_COLS as i32 - c1 * (c1 - 1) / 2;
        let col_pair = col_offset + (c2 - c1);

        (row_pair * K_NUM_COL_PAIRS + col_pair) as usize
    }

    #[inline]
    pub fn get_rect(&self, id: usize) -> &RectInfo {
        &self.rects[id]
    }

    #[inline]
    pub fn get_cells(&self, id: usize) -> &[u8] {
        let info = &self.rects[id];
        let start = info.cell_offset as usize;
        let end = start + info.cell_count as usize;
        &self.cell_table[start..end]
    }
}



// ===== src/moves.rs =====



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



// ===== src/zobrist.rs =====



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



// ===== src/tt.rs =====



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



// ===== src/time_manager.rs =====



/// Detect game phase based on live mushroom count:
/// live > 32 -> Opening, 20-32 -> Midgame, 13-19 -> Late, <= 12 -> Endgame.
pub fn detect_phase(board: &Board) -> GamePhase {
    let live = board.live_count;
    if live > 32 {
        GamePhase::Opening
    } else if live >= 20 {
        GamePhase::Midgame
    } else if live > 12 {
        GamePhase::Late
    } else {
        GamePhase::Endgame
    }
}

#[allow(dead_code)]
pub fn estimate_moves_left(live_count: i32) -> i32 {
    if live_count > 60 {
        22
    } else if live_count > 40 {
        17
    } else if live_count > 25 {
        12
    } else if live_count > 12 {
        8
    } else {
        5
    }
}

fn phase_pct(live_count: i32, _config: &SideConfig) -> f32 {
    // Phase-based % of remaining time per move (from log analysis of winning engines).
    let phase = if live_count > 32 {
        GamePhase::Opening
    } else if live_count >= 20 {
        GamePhase::Midgame
    } else if live_count > 12 {
        GamePhase::Late
    } else {
        GamePhase::Endgame
    };

    match phase {
        GamePhase::Opening => 6.0,
        GamePhase::Midgame => 10.0,
        GamePhase::Late => 12.0,
        GamePhase::Endgame => 18.0,
    }
}

pub struct TimeManager;

impl TimeManager {
    pub fn new() -> Self {
        TimeManager
    }

    fn margin_factor(margin: i32) -> f32 {
        if margin > 40 {
            0.6
        } else if margin > 20 {
            0.7
        } else if margin > 5 {
            0.85
        } else if margin > -5 {
            1.0
        } else if margin > -20 {
            1.2
        } else if margin > -40 {
            1.35
        } else {
            1.5
        }
    }

    /// Per-move budget = remaining_ms * phase_pct/100 * side_mult * margin_factor,
    /// clamped to sane bounds.
    pub fn get_budget(&self, live_count: i32, config: &SideConfig, remaining_ms: i32, margin: i32) -> i32 {
        // Emergency: <500ms remaining -> fixed tiny budget.
        if remaining_ms < 500 {
            return 15;
        }

        let pct = phase_pct(live_count, config);
        let mut budget_f = remaining_ms as f32 * (pct / 100.0);

        // Side multiplier (FIRST=1.0, SECOND=1.5).
        budget_f *= config.time_multiplier;

        // Margin factor (winning=save, losing=invest).
        budget_f *= Self::margin_factor(margin);

        if budget_f < 10.0 {
            budget_f = 10.0;
        }

        // Generous cap: winning bots in logs spend up to 33%+.
        let max_budget = if live_count <= 12 { 2500.0 } else { 2000.0 };
        if budget_f > max_budget {
            budget_f = max_budget;
        }

        // Hard limit: never use > 90% of remaining.
        let hard_limit = remaining_ms as f32 * 0.9;
        if budget_f > hard_limit {
            budget_f = hard_limit;
        }

        budget_f as i32
    }
}



// ===== src/search.rs =====


use std::time::Instant;


const MAX_DEPTH: i32 = 64;
const INF: i32 = 999_999;
const KILLER_SIZE: usize = (MAX_DEPTH as usize) + 1;

/// Static eval weights for geometry-flavored extras. Not currently wired into
/// `evaluate()` (kept for parity with the original `EvalWeights`/`k_default_weights`,
/// which were likewise unused by the hardcoded `evaluate()` multipliers).
#[allow(dead_code)]
pub struct EvalWeights {
    pub mobility: i32,
    pub safe_cell: i32,
    pub steal: i32,
}
#[allow(dead_code)]
pub const K_DEFAULT_WEIGHTS: EvalWeights = EvalWeights { mobility: 1, safe_cell: 1, steal: 2 };

#[derive(Clone, Copy)]
pub struct SearchResult {
    pub mv: Move,
    pub eval: i32,
    pub max_depth: i32,
    pub tt_probes: i64,
    pub tt_hits: i64,
    pub nodes: i64,
}

impl SearchResult {
    fn simple(mv: Move, eval: i32) -> Self {
        SearchResult { mv, eval, max_depth: 0, tt_probes: 0, tt_hits: 0, nodes: 0 }
    }
}

#[allow(dead_code)]
pub struct SearchBenchmark {
    pub avg_depth: f64,
    pub avg_hit_rate: f64,
    pub avg_nodes: f64,
    pub avg_ms: f64,
    pub samples: i32,
}

#[inline]
fn hist_index(mv: Move) -> usize {
    let r1 = mv.r1 as usize;
    let c1 = mv.c1 as usize;
    let r2 = mv.r2 as usize;
    let c2 = mv.c2 as usize;
    ((r1 * K_COLS + c1) * K_ROWS + r2) * K_COLS + c2
}

#[inline]
fn is_futile(_board: &Board, _depth: i32, _alpha: i32) -> bool {
    false
}

pub struct Search {
    table: RectTable,
    zobrist: Zobrist,
    tt: TranspositionTable,

    killer1: [Move; KILLER_SIZE],
    killer2: [Move; KILLER_SIZE],

    start_time: Instant,
    time_limit_ms: i64,
    node_count: i64,
    timed_out: bool,

    tt_probes: i64,
    tt_hits: i64,
    max_depth_reached: i32,

    prefer_vertical: bool,
    aggression: f32,
    #[allow(dead_code)]
    steal_bonus: f32,
    #[allow(dead_code)]
    defense_bonus: f32,

    // History heuristic: flat [r1][c1][r2][c2] table.
    history: Vec<i32>,
}

impl Search {
    pub fn new(table: RectTable, zobrist: Zobrist) -> Self {
        Search {
            table,
            zobrist,
            tt: TranspositionTable::new(18), // 2^18 = 256K entries
            killer1: [ZERO_MOVE; KILLER_SIZE],
            killer2: [ZERO_MOVE; KILLER_SIZE],
            start_time: Instant::now(),
            time_limit_ms: 0,
            node_count: 0,
            timed_out: false,
            tt_probes: 0,
            tt_hits: 0,
            max_depth_reached: 0,
            prefer_vertical: false,
            aggression: 0.5,
            steal_bonus: 1.0,
            defense_bonus: 1.0,
            history: vec![0i32; K_ROWS * K_COLS * K_ROWS * K_COLS],
        }
    }

    // ===== Time management =====

    fn time_check(&mut self) -> bool {
        self.node_count += 1;
        if self.node_count % 1024 == 0 {
            let elapsed = self.start_time.elapsed().as_millis() as i64;
            if elapsed >= self.time_limit_ms {
                self.timed_out = true;
                return false;
            }
        }
        true
    }

    fn history_add(&mut self, mv: Move, val: i32) {
        let idx = hist_index(mv);
        self.history[idx] += val;
    }

    // ===== Move ordering =====

    fn order_score(&self, board: &Board, mv: Move, depth: i32) -> i32 {
        if mv.is_pass() {
            return -1;
        }

        if mv == self.killer1[depth as usize] {
            return 200;
        }
        if mv == self.killer2[depth as usize] {
            return 100;
        }

        let h = self.history[hist_index(mv)];
        if h > 0 {
            return 50 + h;
        }

        if self.prefer_vertical {
            let height = mv.r2 - mv.r1 + 1;
            let width = mv.c2 - mv.c1 + 1;
            if height > width {
                return 25;
            }
        } else if self.aggression < 0.4 {
            let height = mv.r2 - mv.r1 + 1;
            let width = mv.c2 - mv.c1 + 1;
            if width >= height {
                return 25;
            }
        }

        let mut sum = 0i32;
        for r in mv.r1..=mv.r2 {
            for c in mv.c1..=mv.c2 {
                sum += board.value_at(r as i32, c as i32) as i32;
            }
        }
        sum
    }

    fn sort_moves(&self, board: &Board, moves: &mut [Move], depth: i32, tt_move: Move) {
        moves.sort_by(|a, b| {
            let mut sa = self.order_score(board, *a, depth);
            let mut sb = self.order_score(board, *b, depth);
            if *a == tt_move {
                sa = 10000;
            }
            if *b == tt_move {
                sb = 10000;
            }
            sb.cmp(&sa) // descending
        });
    }

    // ===== Root-level geometry enhancement =====

    fn enhance_root_moves(&self, board: &mut Board, moves: &mut [Move], player: i32) {
        for mv in moves.iter_mut() {
            if mv.is_pass() {
                continue;
            }

            let undo = board.apply_move(*mv);
            let mut eval_score = evaluate(board, player);
            board.unmake_move(&undo);

            // Mobility bonus: how many legal moves does the opponent have after our move?
            let opp_moves = generate_legal_moves_optimized(board, &self.table);
            let mobility_bonus = -(opp_moves.len() as i32) / 3;
            eval_score += mobility_bonus;

            // Steal bonus: does this rect contain any opponent cells?
            let rect_id_val = self.table.rect_id(mv.r1 as i32, mv.c1 as i32, mv.r2 as i32, mv.c2 as i32);
            let cells = self.table.get_cells(rect_id_val);
            let mut opp_cells = 0;
            for &cidx in cells {
                if board.owners[cidx as usize] as i32 == -player {
                    opp_cells += 1;
                }
            }
            eval_score += opp_cells * 2;

            mv.score_hint = eval_score;
        }
    }

    // ===== Negamax (original, no geometry) =====

    fn negamax(&mut self, board: &mut Board, depth: i32, alpha: i32, beta: i32, allow_pass: bool) -> i32 {
        if self.timed_out {
            return 0;
        }
        if !self.time_check() {
            return 0;
        }

        let terminal = board.is_terminal();
        if depth <= 0 || terminal {
            return evaluate(board, board.current_player);
        }

        self.tt_probes += 1;
        let hash = self.zobrist.compute(board);
        let mut tt_score = 0;
        let mut tt_move = PASS_MOVE;
        let tt_flag = self.tt.probe(hash, depth, &mut tt_score, &mut tt_move);
        if tt_flag != TTFlag::Empty {
            self.tt_hits += 1;
        }
        if tt_flag == TTFlag::Exact {
            return tt_score;
        }
        if tt_flag == TTFlag::Alpha && tt_score <= alpha {
            return alpha;
        }
        if tt_flag == TTFlag::Beta && tt_score >= beta {
            return beta;
        }

        // Futility pruning: currently always false (kept for parity with the original).
        if is_futile(board, depth, alpha) {
            return evaluate(board, board.current_player);
        }

        let mut alpha = alpha;

        // Null-move pruning.
        if allow_pass && depth >= 3 && !terminal && board.consecutive_passes < 1 {
            let undo = board.apply_move(PASS_MOVE);
            let score = -self.negamax(board, depth - 1 - 2, -beta, -beta + 1, false);
            board.unmake_move(&undo);
            if score >= beta {
                return beta;
            }
        }

        let mut moves = generate_legal_moves_optimized(board, &self.table);
        moves.push(PASS_MOVE);
        self.sort_moves(board, &mut moves, depth, tt_move);

        let mut best_score = -INF;
        let mut best_move = PASS_MOVE;
        let alpha_orig = alpha;
        let mut searched = 0;

        for i in 0..moves.len() {
            let mv = moves[i];
            let undo = board.apply_move(mv);

            let score;
            let mut is_full_search = false;

            if searched >= 4 && depth >= 3 && !mv.is_pass() && mv != tt_move {
                let mut r = 1 + (searched / 4);
                if r > depth / 2 {
                    r = depth / 2;
                }
                if r < 1 {
                    r = 1;
                }

                let mut s = -self.negamax(board, depth - 1 - r, -alpha - 1, -alpha, true);
                if s > alpha && s < beta {
                    s = -self.negamax(board, depth - 1, -beta, -alpha, true);
                    is_full_search = true;
                }
                score = s;
            } else {
                score = -self.negamax(board, depth - 1, -beta, -alpha, true);
                is_full_search = true;
            }

            board.unmake_move(&undo);

            if score > best_score {
                best_score = score;
                best_move = mv;
            }
            searched += 1;

            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                if !mv.is_pass() {
                    if mv != self.killer1[depth as usize] {
                        self.killer2[depth as usize] = self.killer1[depth as usize];
                        self.killer1[depth as usize] = mv;
                    }
                    if is_full_search {
                        self.history_add(mv, depth * depth);
                    }
                }
                break;
            }
        }

        if best_move != moves[0] && !best_move.is_pass() && !moves[0].is_pass() {
            self.history_add(moves[0], -depth);
        }

        let flag = if best_score <= alpha_orig {
            TTFlag::Alpha
        } else if best_score >= beta {
            TTFlag::Beta
        } else {
            TTFlag::Exact
        };
        self.tt.store(hash, depth, flag, best_score, best_move);

        best_score
    }

    // ===== Endgame exact solver =====

    fn negamax_endgame(&mut self, board: &mut Board, alpha: i32, beta: i32, allow_pass: bool) -> i32 {
        if self.timed_out {
            return 0;
        }
        if !self.time_check() {
            return 0;
        }

        let terminal = board.is_terminal();

        let mut moves = generate_legal_moves_optimized(board, &self.table);

        if terminal {
            return evaluate(board, board.current_player);
        }

        if moves.is_empty() {
            if !allow_pass {
                return evaluate(board, board.current_player);
            }
            let undo = board.apply_move(PASS_MOVE);
            let score = -self.negamax_endgame(board, -beta, -alpha, true);
            board.unmake_move(&undo);
            return score;
        }

        self.tt_probes += 1;
        let hash = self.zobrist.compute(board);
        let mut tt_score = 0;
        let mut tt_move = PASS_MOVE;
        let tt_flag = self.tt.probe(hash, 64, &mut tt_score, &mut tt_move);
        if tt_flag != TTFlag::Empty {
            self.tt_hits += 1;
        }
        if tt_flag == TTFlag::Exact {
            return tt_score;
        }
        if tt_flag == TTFlag::Alpha && tt_score <= alpha {
            return alpha;
        }
        if tt_flag == TTFlag::Beta && tt_score >= beta {
            return beta;
        }

        let mut alpha = alpha;

        // Null-move pruning with reduced R in endgame.
        if allow_pass && board.consecutive_passes < 1 && moves.len() > 1 {
            let undo = board.apply_move(PASS_MOVE);
            let score = -self.negamax_endgame(board, -beta, -beta + 1, false);
            board.unmake_move(&undo);
            if score >= beta {
                return beta;
            }
        }

        moves.push(PASS_MOVE);
        self.sort_moves(board, &mut moves, 64, tt_move);

        let mut best_score = -INF;
        let mut best_move = PASS_MOVE;
        let alpha_orig = alpha;

        for i in 0..moves.len() {
            let mv = moves[i];
            let undo = board.apply_move(mv);

            let score;
            if i == 0 {
                // Full search for PV move.
                score = -self.negamax_endgame(board, -beta, -alpha, mv.is_pass());
            } else {
                // Zero-window search for remaining moves.
                let mut s = -self.negamax_endgame(board, -alpha - 1, -alpha, mv.is_pass());
                if s > alpha && s < beta {
                    s = -self.negamax_endgame(board, -beta, -alpha, mv.is_pass());
                }
                score = s;
            }

            board.unmake_move(&undo);

            if score > best_score {
                best_score = score;
                best_move = mv;
            }

            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                break;
            }
        }

        let flag = if best_score <= alpha_orig {
            TTFlag::Alpha
        } else if best_score >= beta {
            TTFlag::Beta
        } else {
            TTFlag::Exact
        };
        self.tt.store(hash, 64, flag, best_score, best_move);

        best_score
    }

    // ===== Iterative deepening =====

    pub fn iterative_deepening(&mut self, board: &mut Board, time_ms: i32, config: &SideConfig) -> SearchResult {
        self.start_time = Instant::now();
        self.time_limit_ms = time_ms as i64;
        self.timed_out = false;
        self.node_count = 0;
        self.tt_probes = 0;
        self.tt_hits = 0;
        self.max_depth_reached = 0;
        self.tt.clear();

        self.prefer_vertical = config.prefer_vertical;
        self.aggression = config.aggression;
        self.steal_bonus = config.steal_bonus;
        self.defense_bonus = config.defense_bonus;

        for v in self.history.iter_mut() {
            *v = 0;
        }
        for i in 0..KILLER_SIZE {
            self.killer1[i] = ZERO_MOVE;
            self.killer2[i] = ZERO_MOVE;
        }

        let mut best_move = PASS_MOVE;
        let mut best_eval = evaluate(board, board.current_player);

        let mut moves = generate_legal_moves_optimized(board, &self.table);
        if moves.is_empty() {
            return SearchResult::simple(PASS_MOVE, best_eval);
        }

        // Root-level geometry enhancement: score moves with eval + geometry.
        let root_player = board.current_player;
        self.enhance_root_moves(board, &mut moves, root_player);

        moves.sort_by(|a, b| b.score_hint.cmp(&a.score_hint));

        if board.consecutive_passes >= 1 {
            let margin = board.score_from_perspective(board.current_player);
            if margin > 0 {
                return SearchResult::simple(PASS_MOVE, best_eval);
            }
        }

        best_move = moves[0];

        let phase = detect_phase(board);
        let endgame = phase == GamePhase::Endgame;

        if endgame && board.live_count <= 12 {
            // Deep endgame: exact solver.
            self.negamax_endgame(board, -INF, INF, true);
            if self.timed_out && best_move.is_pass() {
                best_move = moves[0];
            }
            let hash = self.zobrist.compute(board);
            let mut tt_score = 0;
            let mut tt_move = PASS_MOVE;
            let flag = self.tt.probe(hash, 64, &mut tt_score, &mut tt_move);
            if flag != TTFlag::Empty && !tt_move.is_pass() {
                let mut valid = false;
                for mv in &moves {
                    if mv.r1 == tt_move.r1 && mv.c1 == tt_move.c1 && mv.r2 == tt_move.r2 && mv.c2 == tt_move.c2 {
                        valid = true;
                        break;
                    }
                }
                if valid {
                    best_move = tt_move;
                    best_eval = tt_score;
                }
            }
            self.max_depth_reached = 64;
            return SearchResult {
                mv: best_move,
                eval: best_eval,
                max_depth: self.max_depth_reached,
                tt_probes: self.tt_probes,
                tt_hits: self.tt_hits,
                nodes: self.node_count,
            };
        }

        // Iterative deepening with progressive widening.
        let max_d = MAX_DEPTH;
        let mut last_eval = 0;

        let mut d = 1;
        while d <= max_d && !self.timed_out {
            self.max_depth_reached = d;
            let alpha0 = last_eval - 100;
            let beta0 = last_eval + 100;

            // Using regular negamax — negamax_geo reserved for future optimization.
            let mut score = self.negamax(board, d, alpha0, beta0, true);
            if self.timed_out {
                break;
            }

            if score <= alpha0 {
                score = self.negamax(board, d, -INF, beta0, true);
            } else if score >= beta0 {
                score = self.negamax(board, d, alpha0, INF, true);
            }
            if self.timed_out {
                break;
            }

            last_eval = score;
            best_eval = score;

            let hash = self.zobrist.compute(board);
            let mut tt_score = 0;
            let mut tt_move = PASS_MOVE;
            let flag = self.tt.probe(hash, d, &mut tt_score, &mut tt_move);
            if tt_move != PASS_MOVE && !tt_move.is_pass() && flag != TTFlag::Empty {
                let mut valid = false;
                for mv in &moves {
                    if mv.r1 == tt_move.r1 && mv.c1 == tt_move.c1 && mv.r2 == tt_move.r2 && mv.c2 == tt_move.c2 {
                        valid = true;
                        break;
                    }
                }
                if valid {
                    best_move = tt_move;
                }
            }

            d += 1;
        }

        SearchResult {
            mv: best_move,
            eval: best_eval,
            max_depth: self.max_depth_reached,
            tt_probes: self.tt_probes,
            tt_hits: self.tt_hits,
            nodes: self.node_count,
        }
    }

    // ===== Simple search =====

    #[allow(dead_code)]
    pub fn simple_search(&self, board: &Board, _config: &SideConfig) -> SearchResult {
        let moves = generate_legal_moves_optimized(board, &self.table);
        let player = board.current_player;

        let mut best = SearchResult::simple(PASS_MOVE, -999_999);

        for mv in &moves {
            let mut tmp = board.clone();
            tmp.current_player = player;
            let undo = tmp.apply_move(*mv);
            let score = evaluate(&tmp, player);
            tmp.unmake_move(&undo);

            if score > best.eval {
                best.eval = score;
                best.mv = *mv;
            }
        }

        if moves.is_empty() {
            return SearchResult::simple(PASS_MOVE, evaluate(board, player));
        }

        if board.consecutive_passes >= 1 {
            let margin = board.score_from_perspective(player);
            if margin > 0 {
                return SearchResult::simple(PASS_MOVE, evaluate(board, player));
            }
        }

        best
    }

    // ===== Benchmark =====

    #[allow(dead_code)]
    pub fn benchmark(table: &RectTable, zobrist: &Zobrist, time_ms: i32, samples: i32) -> SearchBenchmark {
        let mut bm = SearchBenchmark { avg_depth: 0.0, avg_hit_rate: 0.0, avg_nodes: 0.0, avg_ms: 0.0, samples };

        for s in 0..samples {
            let mut board = Board::new();
            let target_live = 60 + (s * 7) % 50;
            for i in 0..target_live {
                let idx = ((i * 37 + s * 13).rem_euclid(K_CELLS as i32)) as usize;
                let val = 1 + ((i + s).rem_euclid(9));
                board.values[idx] = val as i8;
            }
            board.current_player = if s % 2 == 0 { K_PLAYER_US } else { K_PLAYER_OPP };
            board.recalc_live_mask();

            let mut search = Search::new(table.clone(), zobrist.clone());
            let start = Instant::now();
            let result = search.iterative_deepening(&mut board, time_ms, &SideConfig::default());
            let elapsed = start.elapsed().as_millis() as f64;

            bm.avg_depth += result.max_depth as f64;
            bm.avg_nodes += result.nodes as f64;
            bm.avg_ms += elapsed;
            if result.tt_probes > 0 {
                bm.avg_hit_rate += (result.tt_hits as f64 / result.tt_probes as f64) * 100.0;
            }
        }

        bm.avg_depth /= samples as f64;
        bm.avg_nodes /= samples as f64;
        bm.avg_ms /= samples as f64;
        bm.avg_hit_rate /= samples as f64;
        bm
    }
}



// ===== src/protocol.rs =====


use std::io::{self, BufRead, Write};


const DATA_FILE: &str = "data.bin";

pub struct PassTracker {
    pub opp_has_passed: bool,
    pub we_have_passed: bool,
    pub last_pass_player: i32,
}

impl PassTracker {
    pub fn new() -> Self {
        PassTracker { opp_has_passed: false, we_have_passed: false, last_pass_player: 0 }
    }

    #[allow(dead_code)]
    pub fn is_game_over(&self) -> bool {
        self.opp_has_passed && self.we_have_passed
    }

    pub fn reset(&mut self) {
        self.opp_has_passed = false;
        self.we_have_passed = false;
        self.last_pass_player = 0;
    }
}

pub struct Protocol {
    board: Board,
    search: Option<Search>,
    pass_tracker: PassTracker,
    our_player: i32,
    i_am_first: bool,
}

impl Protocol {
    pub fn new() -> Self {
        let search = RectTable::load(DATA_FILE).map(|table| {
            let zobrist = Zobrist::new();
            Search::new(table, zobrist)
        });

        Protocol {
            board: Board::new(),
            search,
            pass_tracker: PassTracker::new(),
            our_player: 0,
            i_am_first: false,
        }
    }

    pub fn run(&mut self) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.is_empty() {
                continue;
            }

            if line.starts_with("READY") {
                self.handle_ready(&line);
            } else if line.starts_with("INIT") {
                self.handle_init(&line);
            } else if line.starts_with("TIME") {
                self.handle_time(&line);
            } else if line.starts_with("OPP") {
                self.handle_opp(&line);
            } else if line.starts_with("FINISH") {
                break;
            }
        }
    }

    fn handle_ready(&mut self, line: &str) {
        self.i_am_first = line.contains("FIRST");
        self.our_player = if self.i_am_first { K_PLAYER_US } else { K_PLAYER_OPP };
        self.pass_tracker.reset();
        let mut out = io::stdout();
        let _ = writeln!(out, "OK");
        let _ = out.flush();
    }

    fn handle_init(&mut self, line: &str) {
        let mut it = line.split_whitespace();
        let _cmd = it.next();

        self.board = Board::new();
        for r in 0..K_ROWS {
            let tok = match it.next() {
                Some(t) => t,
                None => break,
            };
            let mut row_val: u64 = match tok.parse() {
                Ok(v) => v,
                Err(_) => break,
            };
            for c in (0..K_COLS).rev() {
                let digit = (row_val % 10) as i8;
                row_val /= 10;
                self.board.values[r * K_COLS + c] = digit;
            }
        }
        self.board.recalc_live_mask();
        self.board.my_mask = Bitboard::empty();
        self.board.opp_mask = Bitboard::empty();
        self.board.current_player = self.our_player;
    }

    fn handle_opp(&mut self, line: &str) {
        let mut it = line.split_whitespace();
        let _cmd = it.next();
        let r1: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let c1: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let r2: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let c2: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let _ms = it.next(); // unused, matches original

        let opp_move = Move { r1: r1 as i8, c1: c1 as i8, r2: r2 as i8, c2: c2 as i8, score_hint: 0 };

        if opp_move.is_pass() {
            // BTC may send duplicate OPP pass lines (logging bug). Skip the duplicate
            // to avoid corrupting board state (consecutive_passes, current_player).
            // Only apply the first real pass.
            if self.pass_tracker.last_pass_player != K_PLAYER_OPP {
                self.pass_tracker.opp_has_passed = true;
                self.pass_tracker.last_pass_player = K_PLAYER_OPP;
                let _ = self.board.apply_move(opp_move);
            }
        } else {
            self.pass_tracker.last_pass_player = 0;
            self.pass_tracker.opp_has_passed = false;
            let _ = self.board.apply_move(opp_move);
        }
    }

    fn handle_time(&mut self, line: &str) {
        let mut it = line.split_whitespace();
        let _cmd = it.next();
        let our_time: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let _opp_time: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);

        let config = SideConfig {
            time_multiplier: if self.i_am_first { 1.0 } else { 1.5 },
            aggression: if self.i_am_first { 0.3 } else { 0.7 },
            steal_bonus: 1.0,
            defense_bonus: if self.i_am_first { 2.0 } else { 1.0 },
            prefer_vertical: !self.i_am_first,
        };

        let tm = TimeManager::new();
        let margin = self.board.score_from_perspective(self.our_player);
        let search_time_ms = tm.get_budget(self.board.live_count, &config, our_time, margin);

        let best = if let Some(search) = self.search.as_mut() {
            let result = search.iterative_deepening(&mut self.board, search_time_ms, &config);
            result.mv
        } else {
            PASS_MOVE
        };

        if best.is_pass() {
            if self.pass_tracker.last_pass_player != self.our_player {
                self.pass_tracker.we_have_passed = true;
                self.pass_tracker.last_pass_player = self.our_player;
            }
        } else {
            self.pass_tracker.reset();
        }

        let _ = self.board.apply_move(best);
        self.write_move(best);
    }

    fn write_move(&self, mv: Move) {
        let mut out = io::stdout();
        if mv.is_pass() {
            let _ = writeln!(out, "-1 -1 -1 -1");
        } else {
            let _ = writeln!(out, "{} {} {} {}", mv.r1, mv.c1, mv.r2, mv.c2);
        }
        let _ = out.flush();
    }
}



// ===== src/main.rs =====


fn main() {
    let mut protocol = Protocol::new();
    protocol.run();
}


