# uasset-lens — Binary Parser Rules

## References

- [byteorder documentation](https://docs.rs/byteorder)
- [UE5 .uasset format](https://docs.rs/unreal-asset) (参考のみ — 実装は自前)

---

## Philosophy

`.uasset` パーサーはこのプロジェクトの核心部品であり、ポートフォリオとしても重要。
- **正確**: 不正な入力をきれいに拒否する
- **回復力**: 破損ファイル 1 つでスキャン全体を止めない
- **ゼロコピー志向**: 可能な限り `&[u8]` スライスで解析し、必要なときだけ所有権を取る

---

## ファイル読み込み戦略

ファイル全体を一度 `Vec<u8>` に読み込んでからスライスを解析する。
`BufReader` + シーク操作を繰り返さない。UE5 アセットのサイズは実用上 50 MB 以下に収まる。

```rust
// ✅ Single read, parse from slice
let data = std::fs::read(path)?;
let metadata = parse_asset(&data, content_root)?;
```

---

## エンディアン

UE5 の `.uasset` ファイルは **常にリトルエンディアン**（macOS 上でも同じ）。
全マルチバイト読み込みに `byteorder::LittleEndian` を使う。ネイティブエンディアンは使わない。

```rust
use byteorder::{LittleEndian, ReadBytesExt};

// ✅ Explicit little-endian
let magic = cursor.read_u32::<LittleEndian>()?;

// ❌ FORBIDDEN — host endianness dependent
let magic = cursor.read_u32::<NativeEndian>()?;
```

---

## byteorder + Cursor パターン

UE5 の `.uasset` はオフセットベースのナビゲーション（NameTable / ImportTable / ExportTable それぞれの
絶対位置をヘッダーから得てジャンプする）が前提のため、ストリーミング消費モデルではなく
`Cursor` を使って絶対位置に直接 `set_position()` する方式を採用している。

### プリミティブ読み込みには `ReadBytesExt` を使う

```rust
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

let mut cur = Cursor::new(data);
let magic   = cur.read_u32::<LittleEndian>().map_err(map_io)?;
let version = cur.read_i32::<LittleEndian>().map_err(map_io)?;
```

### 配列読み込みにはループを使う（with_capacity で事前確保）

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

### I/O エラーは `map_io` でドメインエラーに変換する

```rust
fn map_io(e: std::io::Error) -> ScanError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ScanError::UnexpectedEof
    } else {
        ScanError::Io(e)
    }
}
```

### カーソル移動には `advance()` ヘルパーを使う（bounds check 込み）

直接 `set_position()` を呼ばない。必ず `advance()` 経由で移動し、
バッファ終端を超えた場合に `ScanError::UnexpectedEof` を返す。

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

### パーサー内で `unwrap` を使わない

```rust
// ❌ FORBIDDEN
let val = cur.read_u32::<LittleEndian>().unwrap();

// ✅ map_err でドメインエラーへ変換
let val = cur.read_u32::<LittleEndian>().map_err(map_io)?;
```

---

## Magic Number 検証

パースの最初の操作として必ず Magic Number を検証する。
不一致の場合は即座に `ScanError::InvalidMagic` を返す。

```rust
const UE_MAGIC: u32 = 0x9E2A83C1;

let mut cur = Cursor::new(data);
let magic = cur.read_u32::<LittleEndian>().map_err(map_io)?;
if magic != UE_MAGIC {
    return Err(ScanError::InvalidMagic(magic));
}
```

---

## オフセットベースのテーブルナビゲーション

`FPackageFileSummary` の各テーブル（NameTable / ImportTable / ExportTable）は
ヘッダーに格納されたオフセットで位置が決まる。テーブルの出現順序を仮定しない。

```rust
// ✅ Use header-provided offsets — don't assume order
let name_data   = &data[header.name_offset   as usize..];
let import_data = &data[header.import_offset as usize..];
let export_data = &data[header.export_offset as usize..];
```

---

## FString パース

UE の FString は長さプレフィックス付き。負の長さは UTF-16LE を示す。
Phase 1 ではアセット名として一般的な UTF-8 / ASCII のみ対応する。

```
FString バイナリ形式:
  i32  length  ( < 0 → UTF-16LE;  > 0 → UTF-8/ASCII、null 終端含む長さ;  == 0 → 空文字 )
  [u8] data
```

```rust
fn parse_fstring(cur: &mut Cursor<&[u8]>) -> Result<String, ScanError> {
    let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if len == 0 {
        return Ok(String::new());
    }
    if len < 0 {
        // UTF-16LE: Phase 1 では未対応 — スキップして空文字列を返す
        advance(cur, (-len as u64) * 2)?;
        return Ok(String::new());
    }
    let pos = cur.position() as usize;
    advance(cur, len as u64)?;
    let bytes = &cur.get_ref()[pos..pos + len as usize];
    // null 終端を除いて UTF-8 デコード
    let s = std::str::from_utf8(&bytes[..bytes.len().saturating_sub(1)])
        .map_err(|_| ScanError::InvalidData("invalid UTF-8 in FString".into()))?
        .to_owned();
    Ok(s)
}
```

---

## エラー回復（scan_files レベル）

1 ファイルのパースエラーはスキャン全体を止めない。
破損ファイルは `ScanResult.skipped` に入れ、`warn` ログを出してスキャンを継続する。

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

パースエラーは `warn` レベルでログを出す。`error` レベルは使わない（予期されたケースのため）。

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

## バージョン検証

`FPackageFileSummary` パース後に UE5 ファイルかどうかを確認する。

```rust
// UE5 の条件: legacy_version == -8 かつ file_version_ue5 > 0
if file_version_ue5 <= 0 {
    return Err(ScanError::UnsupportedVersion(
        legacy_version,
        file_version_ue5 as u32,
    ));
}
```

対応バージョン: UE5.1 以降（`legacy_version == -8`）。UE4 ファイルは `UnsupportedVersion` として skipped に入れる。

---

## ImportTable フィルタリング

依存関係として保存するのは `/Game/` プレフィックスのパスのみ。
以下を**変換前にフィルタリング**して `AssetPath` アロケーションを避ける。

| プレフィックス | 例 | 除外理由 |
|---|---|---|
| `/Script/` | `/Script/Engine.StaticMesh` | エンジンクラス定義 |
| `/Engine/` | `/Engine/Content/T_DefaultNormal` | エンジン内蔵コンテンツ |

```rust
// ✅ Filter before allocating AssetPath
let deps: Vec<AssetPath> = resolved_imports.iter()
    .filter(|s| s.starts_with("/Game/"))
    .filter_map(|s| AssetPath::new(s).ok())
    .collect();
```
