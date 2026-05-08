# uasset-lens — Binary Parser Rules

## References

- [nom documentation](https://docs.rs/nom)
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

## nom 使用パターン

### プリミティブ読み込みには `nom::number::complete` を使う

```rust
use nom::number::complete::{le_u32, le_i32, le_i64};

fn parse_magic(input: &[u8]) -> IResult<&[u8], u32> {
    le_u32(input)
}
```

### 固定長配列には `nom::multi::count` を使う

```rust
use nom::multi::count;

fn parse_import_table(input: &[u8], n: usize) -> IResult<&[u8], Vec<FObjectImport>> {
    count(parse_import_entry, n)(input)
}
```

### ドメイン型への変換には `map_res` を使う

```rust
use nom::combinator::map_res;

fn parse_asset_path(input: &[u8]) -> IResult<&[u8], AssetPath> {
    map_res(parse_fstring, AssetPath::new)(input)
}
```

### パーサー内で `unwrap` を使わない

```rust
// ❌ FORBIDDEN
let (rest, val) = le_u32(input).unwrap();

// ✅ ? または明示的なエラーマッピング
let (rest, val) = le_u32(input)
    .map_err(|_| ScanError::UnexpectedEof)?;
```

---

## Magic Number 検証

パースの最初の操作として必ず Magic Number を検証する。
不一致の場合は即座に `ScanError::InvalidMagic` を返す。

```rust
const UE_MAGIC: u32 = 0x9E2A83C1;

fn check_magic(data: &[u8]) -> Result<(), ScanError> {
    if data.len() < 4 {
        return Err(ScanError::UnexpectedEof);
    }
    let magic = u32::from_le_bytes(data[..4].try_into().unwrap());
    if magic != UE_MAGIC {
        return Err(ScanError::InvalidMagic(magic));
    }
    Ok(())
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
fn parse_fstring(input: &[u8]) -> IResult<&[u8], String> {
    let (input, len) = le_i32(input)?;
    if len == 0 {
        return Ok((input, String::new()));
    }
    if len < 0 {
        // UTF-16LE: Phase 1 では未対応 — エラーとして返す
        return Err(nom::Err::Error(
            nom::error::Error::new(input, nom::error::ErrorKind::Tag)
        ));
    }
    let (input, bytes) = take(len as usize)(input)?;
    // null 終端を除いて UTF-8 デコード
    let s = std::str::from_utf8(&bytes[..bytes.len().saturating_sub(1)])
        .map_err(|_| nom::Err::Error(
            nom::error::Error::new(input, nom::error::ErrorKind::Char)
        ))?
        .to_owned();
    Ok((input, s))
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
if !version.is_ue5() {
    return Err(ScanError::UnsupportedVersion(
        version.legacy_version,
        version.file_version_ue5,
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
