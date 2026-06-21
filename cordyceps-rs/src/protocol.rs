//! Line-based stdin/stdout protocol: READY, INIT, TIME, OPP, FINISH.

use std::io::{self, BufRead, Write};

use crate::board::Board;
use crate::bitboard::Bitboard;
use crate::consts::{K_COLS, K_PLAYER_OPP, K_PLAYER_US, K_ROWS};
use crate::rect_table::RectTable;
use crate::search::Search;
use crate::time_manager::TimeManager;
use crate::types::{Move, SideConfig, PASS_MOVE};
use crate::zobrist::Zobrist;

const DATA_FILE: &str = "cordycep.bin";

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
