# uasset-lens — Binary Parser Rules

## References

- [byteorder documentation](https://docs.rs/byteorder)
- [UE5 .uasset format](https://docs.rs/unreal-asset) (reference only — implementation is hand-written)

---

## Philosophy

The `.uasset` parser is the core of this project and an important portfolio piece.
- **Correct**: reject invalid input cleanly
- **Resilient**: a single corrupt file must not halt the entire scan
- **Zero-copy minded**: parse from `&[u8]` slices wherever possible; take ownership only when necessary

---

## File Reading Strategy

Read the entire file into a `Vec<u8>` once, then parse from slices.
Do not use `BufReader` with repeated seeks. UE5 assets are practically always under 50 MB.

```rust
// ✅ Single read, parse from slice
let data = std::fs::read(path)?;
let metadata = parse_asset(&data, content_root)?;
```

---

## Endianness

UE5 `.uasset` files are **always little-endian** (even on macOS).
Use `byteorder::LittleEndian` for all multi-byte reads. Never use native endian.

```rust
use byteorder::{LittleEndian, ReadBytesExt};

// ✅ Explicit little-endian
let magic = cursor.read_u32::<LittleEndian>()?;

// ❌ FORBIDDEN — host endianness dependent
let magic = cursor.read_u32::<NativeEndian>()?;
```

---

## byteorder + Cursor Pattern

UE5 `.uasset` navigation is offset-based: the header provides absolute positions for
NameTable, ImportTable, and ExportTable. Rather than streaming sequentially, use `Cursor`
and jump directly to each table via `set_position()`.

### Use `ReadBytesExt` for primitive reads

```rust
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

let mut cur = Cursor::new(data);
let magic   = cur.read_u32::<LittleEndian>().map_err(map_io)?;
let version = cur.read_i32::<LittleEndian>().map_err(map_io)?;
```

### Use loops for array reads (pre-allocate with with_capacity)

```rust
let count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
if count < 0 {
    return Err(ScanError::InvalidData("negative count".into()));
}
let mut entries = Vec::with_capacity(count as usize);
for _ in 0..count {
    entries.push(parse_entry(&mut cur)?);
}
```

### Convert I/O errors to domain errors with `map_io`

```rust
fn map_io(e: std::io::Error) -> ScanError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ScanError::UnexpectedEof
    } else {
        ScanError::Io(e)
    }
}
```

### Use the `advance()` helper for cursor movement (includes bounds check)

Never call `set_position()` directly. Always move through `advance()`,
which returns `ScanError::UnexpectedEof` when the cursor exceeds the buffer end.

```rust
fn advance(cur: &mut Cursor<&[u8]>, n: u64) -> Result<(), ScanError> {
    let new_pos = cur.position() + n;
    if new_pos > cur.get_ref().len() as u64 {
        return Err(ScanError::UnexpectedEof);
    }
    cur.set_position(new_pos);
    Ok(())
}
```

### Never use `unwrap` inside a parser

```rust
// ❌ FORBIDDEN
let val = cur.read_u32::<LittleEndian>().unwrap();

// ✅ Convert to domain error with map_err
let val = cur.read_u32::<LittleEndian>().map_err(map_io)?;
```

---

## Magic Number Verification

Always verify the Magic Number as the very first parse operation.
Return `ScanError::InvalidMagic` immediately on mismatch.

```rust
const UE_MAGIC: u32 = 0x9E2A83C1;

let mut cur = Cursor::new(data);
let magic = cur.read_u32::<LittleEndian>().map_err(map_io)?;
if magic != UE_MAGIC {
    return Err(ScanError::InvalidMagic(magic));
}
```

---

## Offset-Based Table Navigation

Each table in `FPackageFileSummary` (NameTable, ImportTable, ExportTable) is located
by the offsets stored in the header. Never assume a fixed table order.

```rust
// ✅ Use header-provided offsets — don't assume order
let name_data   = &data[header.name_offset   as usize..];
let import_data = &data[header.import_offset as usize..];
let export_data = &data[header.export_offset as usize..];
```

---

## FString Parsing

UE FStrings are length-prefixed. A negative length signals UTF-16LE encoding.
Phase 1 supports only UTF-8/ASCII, which covers all common asset names.

```
FString binary layout:
  i32  length  ( < 0 → UTF-16LE;  > 0 → UTF-8/ASCII, length includes null terminator;  == 0 → empty )
  [u8] data
```

```rust
fn parse_fstring(cur: &mut Cursor<&[u8]>) -> Result<String, ScanError> {
    let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if len == 0 {
        return Ok(String::new());
    }
    if len < 0 {
        // UTF-16LE: not supported in Phase 1 — skip and return empty string
        advance(cur, (-len as u64) * 2)?;
        return Ok(String::new());
    }
    let pos = cur.position() as usize;
    advance(cur, len as u64)?;
    let bytes = &cur.get_ref()[pos..pos + len as usize];
    // strip null terminator before UTF-8 decode
    let s = std::str::from_utf8(&bytes[..bytes.len().saturating_sub(1)])
        .map_err(|_| ScanError::InvalidData("invalid UTF-8 in FString".into()))?
        .to_owned();
    Ok(s)
}
```

---

## Error Recovery (scan_files level)

A parse error in one file must not halt the entire scan.
Put corrupt files in `ScanResult.skipped`, emit a `warn` log, and continue scanning.

```rust
// ✅ Per-file error isolation with rayon
use rayon::prelude::*;

let (assets, skipped): (Vec<_>, Vec<_>) = files
    .par_iter()
    .map(|path| parse_file(path, content_root)
        .map_err(|e| SkippedFile { path: path.clone(), reason: e })
    )
    .partition_map(|r| match r {
        Ok(meta) => itertools::Either::Left(meta),
        Err(skip) => itertools::Either::Right(skip),
    });
```

Log parse errors at `warn` level. Do not use `error` — skipped files are an expected outcome.

```rust
for skip in &skipped {
    tracing::warn!(
        path = %skip.path.display(),
        reason = %skip.reason,
        "Skipping file"
    );
}
```

---

## Version Verification

Verify the file is UE5 after parsing `FPackageFileSummary`.

```rust
// UE5 condition: legacy_version == -8 and file_version_ue5 > 0
if file_version_ue5 <= 0 {
    return Err(ScanError::UnsupportedVersion(
        legacy_version,
        file_version_ue5 as u32,
    ));
}
```

Supported versions: UE5.1 and later (`legacy_version == -8`). UE4 files become `UnsupportedVersion` entries in `skipped`.

---

## ImportTable Filtering

Only paths with the `/Game/` prefix are stored as dependencies.
**Filter before conversion** to avoid allocating discarded `AssetPath` values.

| Prefix | Example | Reason for exclusion |
|--------|---------|---------------------|
| `/Script/` | `/Script/Engine.StaticMesh` | Engine class definitions |
| `/Engine/` | `/Engine/Content/T_DefaultNormal` | Engine built-in content |

```rust
// ✅ Filter before allocating AssetPath
let deps: Vec<AssetPath> = resolved_imports.iter()
    .filter(|s| s.starts_with("/Game/"))
    .filter_map(|s| AssetPath::new(s).ok())
    .collect();
```
