# `crates/scanner` — NameTable parser

## Summary

Implement `parser/name_table.rs` to read all FString entries from the name table
section of a `.uasset` binary into a `Vec<String>`.
Complete when the name tables of all valid test fixtures are parsed to the expected string lists.

## Design Notes

**FString format (UE5):**

```
length: i32   — byte count including null terminator (positive = UTF-8, negative = UTF-16LE)
data:  [u8]   — `length` bytes (for positive case)
\0            — null terminator (included in `length`, must be stripped from the result)
```

**After each FString**, UE5 name table entries include a **4-byte hash suffix** (`u16 + u16`)
that must be consumed and discarded.

**Negative length (UTF-16LE):** Phase 1 does not implement UTF-16LE decoding.
Emit `tracing::warn!("UTF-16 name entry skipped")`, push an empty `String`, and continue.
Do not return an error — scanning must continue for the rest of the file.

**Function signature:**

```rust
pub fn parse_name_table(
    data: &[u8],
    offset: u64,
    count: usize,
) -> Result<Vec<String>, ScanError>
```

Use `std::io::Cursor` for offset-based reads.

## Requirements

- [ ] Implement `parse_name_table(data, offset, count) -> Result<Vec<String>, ScanError>`
- [ ] Seek to `offset` in a `Cursor<&[u8]>` before reading
- [ ] Parse each FString: read `i32` length, then read bytes, strip null terminator
- [ ] Handle negative length: emit `tracing::warn!`, push `String::new()`, advance cursor, continue
- [ ] Read and discard the 4-byte hash suffix after each FString
- [ ] Unit test: known byte sequence → expected `Vec<String>` (craft inline bytes for a 2–3 entry name table)
- [ ] Unit test: fixture with known names produces the expected first few entries
- [ ] Unit test: negative-length entry → empty string, no panic, parsing continues for subsequent entries

## Related

- Depends on: #5 (ScanError, header offsets)
- Next: #7 — ImportTable parser
- Docs: `docs/rules/binary-parser.md` (FString parsing section)
