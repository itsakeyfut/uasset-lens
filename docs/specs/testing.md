# テスト戦略

## 方針

`.uasset` ファイルは UE 開発において通常 VCS（Git / SVN）にコミットされるバイナリアセットである。
同様に、uasset-lens のテストフィクスチャも実際の `.uasset` ファイルをリポジトリにコミットして使用する。
これにより、UE のインストールなしに CI（マルチ OS）でパーサーの動作を検証できる。

---

## フィクスチャ配置

```
tests/
  fixtures/
    valid/
      BP_Simple.uasset          # Blueprint（Import あり・Export あり）
      T_Rock_D.uasset            # Texture2D
      SM_Cube.uasset             # StaticMesh
      M_Basic.uasset             # Material
      OldName.uasset             # ObjectRedirector
      L_TestMap.umap             # World
    invalid/
      bad_magic.bin              # 先頭 4 バイトが不正（0x00000000）
      truncated.bin              # ヘッダー途中で終端
```

- `valid/` 配下: 実際の UE5 プロジェクトからエクスポートした最小構成の .uasset / .umap
- `invalid/` 配下: エラーケース用の合成バイナリ（テストコード内でインラインに定義してもよい）

---

## テスト分類

### ユニットテスト（`#[cfg(test)]`）

各パーサーモジュール内に記述する。対象は個別のパース関数。

```rust
// crates/scanner/src/parser/header.rs
#[cfg(test)]
mod tests {
    #[test]
    fn rejects_invalid_magic() {
        let data = &[0x00u8; 32];
        assert!(matches!(parse_header(data), Err(ScanError::InvalidMagic(_))));
    }
}
```

### 統合テスト（`tests/` ディレクトリ）

crate ルートの `tests/` に配置し、公開 API を通じて実フィクスチャを使ってテストする。

```rust
// crates/scanner/tests/integration.rs
#[test]
fn parses_blueprint_fixture() {
    let root    = Path::new("tests/fixtures/valid");
    let results = scanner::scan_files(&[root.join("BP_Simple.uasset")], root);
    let meta    = &results.assets[0];
    assert_eq!(meta.asset_type, AssetType::Blueprint);
    assert!(!meta.dependencies.is_empty());
}
```

### エラーケース

エラーケース用フィクスチャは `tests/fixtures/invalid/` に配置するか、
テストコード内で直接バイト列として定義する（短い場合は後者を推奨）。

```rust
#[test]
fn rejects_truncated_file() {
    let data = b"\xC1\x83\x2A\x9E"; // magic のみ、残り欠損
    let path = write_temp_file(data);
    let result = scanner::scan_files(&[path], Path::new("/"));
    assert_eq!(result.skipped.len(), 1);
    assert!(matches!(result.skipped[0].reason, ScanError::UnexpectedEof));
}
```

---

## CI 設定

- マルチ OS テスト: Windows / macOS / Linux（GitHub Actions matrix）
- フィクスチャは通常の `git checkout` で取得できるためセットアップ不要
- `.uasset` ファイルは `.gitattributes` で `binary` 属性を付与し、LF 変換を防ぐ

```
# .gitattributes
tests/fixtures/**/*.uasset binary
tests/fixtures/**/*.umap   binary
```

---

## フィクスチャのメンテナンス

- UE のアップデートで `.uasset` フォーマットが変わった場合、フィクスチャを再生成して PR でコミットする
- フィクスチャの元プロジェクトは `tests/fixtures/README.md` に UE バージョン・生成手順を記載する
