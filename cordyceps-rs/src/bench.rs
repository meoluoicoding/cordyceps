mod bitboard;
mod board;
mod consts;
mod moves;
mod prefix_sum;
mod protocol;
mod rect_table;
mod search;
mod time_manager;
mod tt;
mod types;
mod zobrist;

use std::time::Instant;

use board::Board;
use consts::{K_CELLS, K_PLAYER_US};
use rect_table::RectTable;
use search::Search;
use types::SideConfig;
use zobrist::Zobrist;

const DATA_FILE: &str = "cordycep.bin";

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        XorShift64 { state: if seed == 0 { 1 } else { seed } }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_range(&mut self, max: u64) -> u64 {
        self.next_u64() % max
    }
}

fn generate_random_board(rng: &mut XorShift64, live_count: usize) -> Board {
    let mut board = Board::new();

    let mut indices: Vec<usize> = (0..K_CELLS).collect();
    for i in (1..indices.len()).rev() {
        let j = rng.next_range(i as u64 + 1) as usize;
        indices.swap(i, j);
    }

    let n = live_count.min(K_CELLS);
    for i in 0..n {
        let idx = indices[i];
        let val = (rng.next_range(9) + 1) as i8;
        board.values[idx] = val;
    }

    board.recalc_live_mask();
    board.current_player = K_PLAYER_US;
    board
}

struct BenchResult {
    live_count: usize,
    time_ms: u128,
    nodes: i64,
    max_depth: i32,
    tt_probes: i64,
    tt_hits: i64,
    eval: i32,
    best_move: String,
}

fn run_single(table: &RectTable, zobrist: &Zobrist, board: &mut Board, time_ms: i32, live_count: usize) -> BenchResult {
    let config = SideConfig::default();
    let mut search = Search::new(table.clone(), zobrist.clone());

    let start = Instant::now();
    let result = search.iterative_deepening(board, time_ms, &config);
    let elapsed = start.elapsed();

    let mv_str = if result.mv.is_pass() {
        "PASS".to_string()
    } else {
        format!("({},{})-({},{})", result.mv.r1, result.mv.c1, result.mv.r2, result.mv.c2)
    };

    BenchResult {
        live_count,
        time_ms: elapsed.as_millis(),
        nodes: result.nodes,
        max_depth: result.max_depth,
        tt_probes: result.tt_probes,
        tt_hits: result.tt_hits,
        eval: result.eval,
        best_move: mv_str,
    }
}

fn main() {
    let table = match RectTable::load(DATA_FILE) {
        Some(t) => t,
        None => {
            eprintln!("ERROR: cannot load {}", DATA_FILE);
            std::process::exit(1);
        }
    };
    let zobrist = Zobrist::new();

    let time_budget_ms = 10000;

    let scenarios: Vec<(usize, &str)> = vec![
        (10, "endgame"),
        (20, "late"),
        (30, "midgame"),
        (50, "opening"),
        (80, "early"),
        (120, "full"),
    ];

    let num_samples = 5;

    println!("=== Cordyceps Search Benchmark ===");
    println!("Time budget: {} ms per move", time_budget_ms);
    println!("Samples per scenario: {}", num_samples);
    println!();
    println!("{:<10} {:>6} {:>8} {:>6} {:>10} {:>8} {:>8} {:>8} {:>6} {:>14}",
        "scenario", "live", "time_ms", "depth", "nodes", "nps", "tt_probes", "tt_hits", "eval", "best_move");
    println!("{}", "-".repeat(110));

    let mut totals_time: u128 = 0;
    let mut totals_nodes: i64 = 0;
    let mut totals_depth: i64 = 0;
    let mut count: i64 = 0;

    let base_seed: u64 = 42;

    for &(live, label) in &scenarios {
        for s in 0..num_samples {
            let seed = base_seed + (live as u64) * 1000 + s as u64;
            let mut rng = XorShift64::new(seed);
            let mut board = generate_random_board(&mut rng, live);

            let res = run_single(&table, &zobrist, &mut board, time_budget_ms, live);

            let nps = if res.time_ms > 0 {
                res.nodes as f64 / (res.time_ms as f64 / 1000.0)
            } else {
                0.0
            };

            println!("{:<10} {:>6} {:>8} {:>6} {:>10} {:>8.0} {:>8} {:>8} {:>6} {:>14}",
                label,
                res.live_count,
                res.time_ms,
                res.max_depth,
                res.nodes,
                nps,
                res.tt_probes,
                res.tt_hits,
                res.eval,
                res.best_move,
            );

            totals_time += res.time_ms;
            totals_nodes += res.nodes;
            totals_depth += res.max_depth as i64;
            count += 1;
        }
    }

    println!("{}", "-".repeat(110));
    let avg_time = totals_time as f64 / count as f64;
    let avg_nodes = totals_nodes as f64 / count as f64;
    let avg_depth = totals_depth as f64 / count as f64;
    let avg_nps = if avg_time > 0.0 { avg_nodes / (avg_time / 1000.0) } else { 0.0 };

    println!();
    println!("=== Summary ===");
    println!("Total runs:    {}", count);
    println!("Avg time:      {:.1} ms", avg_time);
    println!("Avg nodes:     {:.0}", avg_nodes);
    println!("Avg depth:     {:.1}", avg_depth);
    println!("Avg NPS:       {:.0}", avg_nps);
    println!("Total time:    {} ms", totals_time);
}
