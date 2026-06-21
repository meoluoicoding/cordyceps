# Cordyceps (Rust port)

A 1:1 Rust port of the original C++ `cordyceps` bot: a competitive bot for a
10x17 "sum-to-10 rectangle" board game, using bitboards, negamax alpha-beta
search with iterative deepening, a transposition table, killer moves +
history heuristic, null-move pruning, late-move reductions, and an exact
endgame solver — talking to a judge over a line-based stdin/stdout protocol
(`READY` / `INIT` / `TIME` / `OPP` / `FINISH`).

## Build

```bash
cargo build --release
```

The binary is produced at `target/release/cordyceps`.

## Run

The bot needs the precomputed rectangle-geometry table **`data.bin`** in its
current working directory (same file/format your original C++ build used —
the loader reads the identical binary layout, see `src/rect_table.rs`). Copy
it next to the binary (or `cd` into that directory) before running:

```bash
cp /path/to/your/data.bin .
./target/release/cordyceps
```

It then speaks the same line protocol as the original on stdin/stdout.

## Project layout

| File                  | Mirrors (from `main.cpp`)                                   |
|------------------------|---------------------------------------------------------------|
| `src/consts.rs`        | Board-size / player constants                                 |
| `src/types.rs`         | `Move`, `SideConfig`, `GamePhase`                              |
| `src/bitboard.rs`      | `Bitboard` (3x u64, 170-cell board)                            |
| `src/board.rs`         | `Board`, `EvalCache`, `UndoMove`, `evaluate()`, tune weights   |
| `src/rect_table.rs`    | `RectTable` — loads `data.bin`                                 |
| `src/prefix_sum.rs`    | `PrefixSum` — O(1) rectangle sums                              |
| `src/moves.rs`         | Legal move generation (brute force + optimized)                |
| `src/zobrist.rs`       | Zobrist hashing                                                |
| `src/tt.rs`            | Transposition table                                            |
| `src/time_manager.rs`  | Game-phase detection + per-move time budget                    |
| `src/search.rs`        | `Search`: negamax, endgame solver, iterative deepening         |
| `src/protocol.rs`      | stdin/stdout protocol handler                                  |
| `src/main.rs`          | Entry point                                                    |

## Notes on the port (read before relying on it)

I translated this **by hand, very carefully, but could not compile or run it**
in the sandbox I wrote it in (no Rust toolchain / no internet access there).
I did do brace/paren/bracket balance checks and a careful manual review of
every borrow, but **please run `cargo build` yourself and paste me any errors
— I'll fix them immediately.** A few deliberate, documented deviations from
the C++ source:

1. **Killer-move array size.** The original sizes `killer1_`/`killer2_` as
   `Move[64]`, but `depth` can reach exactly `64` at the search root, which
   would index one element past the end (undefined behavior in C++, a hard
   panic/crash in Rust). I sized the Rust arrays to 65 slots to fix this —
   it has no effect on search decisions, just prevents a crash.
2. **Uninitialized `tt_move`.** In a few spots the C++ leaves `Move tt_move;`
   uninitialized when the TT probe misses, then still compares it against
   real moves shortly after (relying on a "happens to be harmless" garbage
   read). Rust requires initialization, so I initialize it to the pass-move
   sentinel in those spots — behaviorally a no-op since the surrounding code
   already gates on the probe's flag.
3. **Zobrist RNG.** The original seeds `std::mt19937_64` with a fixed
   constant. Since these random values never leave the process (no
   persistence/cross-language sharing), I used a small dependency-free
   SplitMix64 generator with the same seed instead of reimplementing
   MT19937-64 bit-for-bit — it gives the same property that matters (a fixed,
   reproducible hash space per run) without an external RNG crate.
3. **Move ordering tie-breaks.** `std::sort` in C++ is not stable; Rust's
   `sort_by` is. For moves with identical sort keys, the original and the
   port may pick a different (but equally-scored) order. This cannot affect
   correctness, only which equally-good move is picked in a tie.
4. **`UndoMove` representation.** Internally stores changed cells as
   `Vec<(index, old_value)>` instead of two parallel fixed arrays + a
   counter — purely a representational simplification, identical data/order.

Everything else (board representation, move generation, evaluation weights,
search algorithm including LMR/null-move/aspiration-window iterative
deepening, the exact endgame solver, time management formulas, and the
stdin/stdout protocol) is a direct, line-by-line translation of the C++
logic, including the same constants (`data.bin` magic number `0x43524459`,
PackedRect's 104-byte layout, TT size `2^18`, etc.) — your `data.bin` should
load unmodified.

`gen_geometry` (whatever tool produced `data.bin`) was not part of the
uploaded `main.cpp`, so it isn't ported here — reuse your existing `data.bin`.
