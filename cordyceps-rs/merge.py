#!/usr/bin/env python3
"""Merge all .rs source files into a single submission.rs."""

import re

FILES = [
    "src/consts.rs",
    "src/types.rs",
    "src/bitboard.rs",
    "src/prefix_sum.rs",
    "src/board.rs",
    "src/rect_table.rs",
    "src/moves.rs",
    "src/zobrist.rs",
    "src/tt.rs",
    "src/time_manager.rs",
    "src/search.rs",
    "src/protocol.rs",
    "src/main.rs",
]

LOCAL_MODULES = {
    'board', 'bitboard', 'consts', 'moves', 'prefix_sum',
    'protocol', 'rect_table', 'search', 'time_manager',
    'tt', 'types', 'zobrist',
}

def process(content, filename):
    lines = content.split('\n')
    result = []
    
    for line in lines:
        stripped = line.strip()
        
        # Skip module-level doc comments
        if stripped.startswith('//!'):
            continue
        
        # Skip mod declarations
        if re.match(r'^\s*mod\s+\w+;\s*$', line):
            continue
        
        # Skip use statements that reference local modules
        # e.g. use board::Board; or use crate::board::Board;
        use_match = re.match(r'^\s*use\s+(crate::)?(\w+)::.*;\s*$', line)
        if use_match and use_match.group(2) in LOCAL_MODULES:
            continue
        
        # Remove crate:: prefix from remaining paths
        line = line.replace('crate::', '')
        
        # Remove inline local module prefixes (e.g. consts::K_CELLS -> K_CELLS)
        for mod_name in LOCAL_MODULES:
            line = re.sub(r'\b' + mod_name + r'::', '', line)
        
        # Remove pub(crate) -> pub
        line = line.replace('pub(crate)', 'pub')
        
        result.append(line)
    
    return '\n'.join(result)

def main():
    parts = []
    
    for filepath in FILES:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
        
        processed = process(content, filepath)
        parts.append(f"// ===== {filepath} =====\n")
        parts.append(processed)
        parts.append("\n")
    
    with open('submission.rs', 'w', encoding='utf-8') as f:
        f.write('\n'.join(parts))
    
    print("Written submission.rs")

if __name__ == '__main__':
    main()
