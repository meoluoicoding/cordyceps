//! Precomputed rectangle geometry table, loaded from `data.bin`.
//!
//! Binary layout (matches the original `gen_geometry`/C++ `RectTable::load` format):
//!   u32 magic ("CRDY" packed as 0x43524459)
//!   u32 num_rects
//!   u32 cell_table_size
//!   u32 checksum (read but not verified, matching the original)
//!   num_rects x PackedRect (106 bytes each, all little-endian):
//!       i8 r1, c1, r2, c2
//!       u16 cell_count, u32 cell_offset
//!       u64 top_lo, top_mid, top_hi
//!       u64 bottom_lo, bottom_mid, bottom_hi
//!       u64 left_lo, left_mid, left_hi
//!       u64 right_lo, right_mid, right_hi
//!   cell_table_size bytes of cell table

use std::convert::TryInto;
use std::fs::File;
use std::io::Read;

use crate::bitboard::Bitboard;
use crate::consts::{K_COLS, K_ROWS};

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
