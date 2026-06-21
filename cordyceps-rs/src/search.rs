//! Negamax alpha-beta search with iterative deepening, a transposition table,
//! killer moves, history heuristic, null-move pruning, late-move reduction,
//! and a dedicated exact endgame solver.

use std::time::Instant;

use crate::board::{evaluate, Board};
use crate::consts::{K_COLS, K_PLAYER_OPP, K_ROWS};
use crate::moves::generate_legal_moves_optimized;
use crate::rect_table::RectTable;
use crate::time_manager::detect_phase;
use crate::tt::{TTFlag, TranspositionTable};
use crate::types::{GamePhase, Move, SideConfig, PASS_MOVE, ZERO_MOVE};
use crate::zobrist::Zobrist;

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
            let height = (mv.r2 - mv.r1 + 1) as i32;
            let width = (mv.c2 - mv.c1 + 1) as i32;
            if height > width {
                return 25;
            }
        } else if self.aggression < 0.4 {
            let height = (mv.r2 - mv.r1 + 1) as i32;
            let width = (mv.c2 - mv.c1 + 1) as i32;
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
        let is_second = player == K_PLAYER_OPP;
        let claimed_cells = board.eval_cache.my_territory + board.eval_cache.opp_territory;
        let opening_phase = claimed_cells < 24;
        let midgame_phase = claimed_cells < 70;

        for mv in moves.iter_mut() {
            if mv.is_pass() {
                if is_second {
                    mv.score_hint -= 2000;
                }
                continue;
            }

            let undo = board.apply_move(*mv);
            let mut eval_score = evaluate(board, player);
            board.unmake_move(&undo);

            // Steal bonus
            let rect_id_val = self.table.rect_id(mv.r1 as i32, mv.c1 as i32, mv.r2 as i32, mv.c2 as i32);
            let cells = self.table.get_cells(rect_id_val);
            let mut opp_cells = 0;
            for &cidx in cells {
                if board.owners[cidx as usize] as i32 == -player {
                    opp_cells += 1;
                }
            }
            eval_score += opp_cells * 2;

            // mushroom-bot second defensive biases
            if is_second {
                let mut steal = 0i32;
                let area = ((mv.r2 - mv.r1 + 1) * (mv.c2 - mv.c1 + 1)) as i32;
                for r in mv.r1..=mv.r2 {
                    for c in mv.c1..=mv.c2 {
                        let idx = (r as usize) * K_COLS + (c as usize);
                        if board.owners[idx] as i32 == -player {
                            let adj_live = (r > 0 && board.values[((r - 1) as usize) * K_COLS + c as usize] > 0)
                                || (r < K_ROWS as i8 - 1 && board.values[((r + 1) as usize) * K_COLS + c as usize] > 0)
                                || (c > 0 && board.values[r as usize * K_COLS + (c - 1) as usize] > 0)
                                || (c < K_COLS as i8 - 1 && board.values[r as usize * K_COLS + (c + 1) as usize] > 0);
                            if adj_live { steal += 1; }
                        }
                    }
                }

                let connection = board.connectivity_boost(mv.r1 as i32, mv.c1 as i32, mv.r2 as i32, mv.c2 as i32);
                let risk = board.dead_cell_risk_proxy(mv.r1 as i32, mv.c1 as i32, mv.r2 as i32, mv.c2 as i32);
                let barrier = board.barrier_potential(mv.r1 as i32, mv.c1 as i32, mv.r2 as i32, mv.c2 as i32);

                let compact_bonus = if area <= 4 { 140 } else if area >= 8 { -100 } else { 40 };
                let counter_bonus = if steal > 0 {
                    if opening_phase { steal * 85 } else { steal * 60 }
                } else { 0 };
                let connection_bonus = if opening_phase { connection * 32 } else { connection * 22 };
                let risk_penalty = if opening_phase { risk * 85 } else { risk * 52 };
                let barrier_bonus = if opening_phase { barrier * 180 } else { barrier * 110 };
                let extension_penalty = if steal == 0 && area >= 7 { 140 } else { 0 };
                let loose_shape_penalty = if connection == 0 && risk > 0 { 120 } else { 0 };
                let quiet_opening_penalty = if opening_phase && steal == 0 && barrier == 0 { area * 8 } else { 0 };
                let midgame_guard_bonus = if midgame_phase && connection > 0 && risk == 0 { 90 } else { 0 };

                eval_score += counter_bonus;
                eval_score += connection_bonus;
                eval_score -= risk_penalty;
                eval_score += barrier_bonus;
                eval_score += compact_bonus;
                eval_score += midgame_guard_bonus;
                eval_score -= extension_penalty;
                eval_score -= loose_shape_penalty;
                eval_score -= quiet_opening_penalty;
            }

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
                let idx = ((i * 37 + s * 13).rem_euclid(crate::consts::K_CELLS as i32)) as usize;
                let val = 1 + ((i + s).rem_euclid(9));
                board.values[idx] = val as i8;
            }
            board.current_player = if s % 2 == 0 { crate::consts::K_PLAYER_US } else { crate::consts::K_PLAYER_OPP };
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
