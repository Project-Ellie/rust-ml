//! Stride-16 bitboard over `[u64; 4]` — internal only (`pub(crate)`).
//! Slice 3. Invariant: padding bits (column 15 of each row, bits 240-255)
//! are always zero; complements must immediately AND with `VALID`.
