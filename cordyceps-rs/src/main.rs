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

fn main() {
    let mut protocol = protocol::Protocol::new();
    protocol.run();
}
