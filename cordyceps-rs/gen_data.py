#!/usr/bin/env python3
"""Generate data.bin for cordyceps-rs rectangle geometry table."""

import struct

K_ROWS = 10
K_COLS = 17
K_CELLS = K_ROWS * K_COLS  # 170
MAGIC = 0x43524459  # "CRDY"
# Layout: i8*4 + u16 count + u32 offset + u64*12 masks = 4+2+4+96 = 106
PACKED_RECT_SIZE = 106

def cell_bitboard(cells_set):
    """Convert a set of cell indices to (lo, mid, hi) u64 triple."""
    lo = 0
    mid = 0
    hi = 0
    for idx in cells_set:
        if idx < 64:
            lo |= (1 << idx)
        elif idx < 128:
            mid |= (1 << (idx - 64))
        else:
            hi |= (1 << (idx - 128))
    return lo, mid, hi

def rect_id(r1, c1, r2, c2):
    """O(1) rectangle id matching Rust rect_table.rs formula."""
    K_NUM_COL_PAIRS = K_COLS * (K_COLS + 1) // 2  # 153
    row_offset = r1 * K_ROWS - r1 * (r1 - 1) // 2
    row_pair = row_offset + (r2 - r1)
    col_offset = c1 * K_COLS - c1 * (c1 - 1) // 2
    col_pair = col_offset + (c2 - c1)
    return row_pair * K_NUM_COL_PAIRS + col_pair

def main():
    # Generate all rectangles
    rects = []  # (r1, c1, r2, c2, cells_list)
    cell_table = bytearray()
    
    # Build rects in rect_id order
    all_rects = {}
    
    for r1 in range(K_ROWS):
        for r2 in range(r1, K_ROWS):
            for c1 in range(K_COLS):
                for c2 in range(c1, K_COLS):
                    rid = rect_id(r1, c1, r2, c2)
                    cells = []
                    for r in range(r1, r2 + 1):
                        for c in range(c1, c2 + 1):
                            cells.append(r * K_COLS + c)
                    all_rects[rid] = (r1, c1, r2, c2, cells)
    
    num_rects = len(all_rects)
    print(f"Total rectangles: {num_rects}")
    
    # Build cell table and packed rects
    packed_rects = bytearray()
    current_offset = 0
    
    for rid in range(num_rects):
        r1, c1, r2, c2, cells = all_rects[rid]
        cell_count = len(cells)
        cell_offset = current_offset
        
        # Top edge cells: row=r1, cols c1..c2
        top_cells = set(r * K_COLS + c for c in range(c1, c2 + 1) for r in [r1])
        # Bottom edge cells: row=r2, cols c1..c2
        bottom_cells = set(r * K_COLS + c for c in range(c1, c2 + 1) for r in [r2])
        # Left edge cells: col=c1, rows r1..r2
        left_cells = set(r * K_COLS + c for r in range(r1, r2 + 1) for c in [c1])
        # Right edge cells: col=c2, rows r1..r2
        right_cells = set(r * K_COLS + c for r in range(r1, r2 + 1) for c in [c2])
        
        top_lo, top_mid, top_hi = cell_bitboard(top_cells)
        bot_lo, bot_mid, bot_hi = cell_bitboard(bottom_cells)
        left_lo, left_mid, left_hi = cell_bitboard(left_cells)
        right_lo, right_mid, right_hi = cell_bitboard(right_cells)
        
        # Pack: 106 bytes
        # i8 r1, c1, r2, c2  (4 bytes)
        # u16 cell_count (2 bytes) + u32 cell_offset (4 bytes)
        # 4 x (3 x u64) masks = 4 x 24 = 96 bytes
        # Total: 4 + 6 + 96 = 106
        rect_data = struct.pack('<bbbb', r1, c1, r2, c2)
        rect_data += struct.pack('<HI', cell_count, cell_offset)
        rect_data += struct.pack('<QQQ', top_lo, top_mid, top_hi)
        rect_data += struct.pack('<QQQ', bot_lo, bot_mid, bot_hi)
        rect_data += struct.pack('<QQQ', left_lo, left_mid, left_hi)
        rect_data += struct.pack('<QQQ', right_lo, right_mid, right_hi)
        
        assert len(rect_data) == PACKED_RECT_SIZE, f"Expected {PACKED_RECT_SIZE}, got {len(rect_data)}"
        packed_rects += rect_data
        
        # Add cells to cell table
        for cidx in cells:
            cell_table.append(cidx & 0xFF)
        current_offset += cell_count
    
    cell_table_size = len(cell_table)
    print(f"Cell table size: {cell_table_size}")
    
    # Header: magic(4) + num_rects(4) + cell_table_size(4) + checksum(4)
    header = struct.pack('<IIII', MAGIC, num_rects, cell_table_size, 0)
    
    # Write file
    with open('data.bin', 'wb') as f:
        f.write(header)
        f.write(packed_rects)
        f.write(cell_table)
    
    total_size = len(header) + len(packed_rects) + len(cell_table)
    print(f"Written data.bin: {total_size} bytes")
    print(f"  Header: {len(header)} bytes")
    print(f"  Rects: {len(packed_rects)} bytes ({num_rects} x {PACKED_RECT_SIZE})")
    print(f"  Cell table: {len(cell_table)} bytes")

if __name__ == '__main__':
    main()
