# CLI 詳細設計

## DB ファイルの配置

`<project_dir>/.uasset-lens/uasset-lens.db` に自動生成される。

```
/MyProject/
  ├─ Content/           # UE コンテンツ
  ├─ .uasset-lens/
  │   └─ uasset-lens.db  # ← scan 時に自動生成（.gitignore に追加推奨）
  └─ .uasset-lens.toml  # 設定ファイル（任意）
```

`--db <path>` フラグで任意のパスに上書きできる（CI 用途等）。

## Content ルートの解決ルール

`<project_dir>` の解釈:

1. `<project_dir>/Content/` が存在する → `content_root = <project_dir>/Content/`
2. 存在しない → `content_root = <project_dir>`（Content ディレクトリを直接渡した場合）

`impact` コマンド等でアセットパスのみ渡された場合は、パスを上方に辿って `.uasset-lens/uasset-lens.db` を自動検索する。

## scan 未実行時の挙動

DB が存在しない・空の状態で他のコマンドを実行した場合はエラーで終了する。

```
Error: no scan data found.
Run 'uasset-lens scan <project_dir>' first.
```

## Exit codes

Clippy スタイルの 3 値。CI での品質ゲートとして利用できる。

| コード | 意味 |
|---|---|
| `0` | 正常終了・問題なし |
| `1` | 問題を検出（dead asset・循環依存・impact あり 等） |
| `2` | 実行エラー（IO エラー・DB 未作成・パース失敗 等） |

```bash
# CI での利用例
uasset-lens graph --cycles-only ./Project || exit 1   # 循環依存でビルド失敗
uasset-lens dead-assets ./Project                     # 検出時は exit 1（警告扱い）
```

## 共通フラグ

| フラグ | 説明 |
|---|---|
| `--format <text\|json>` | 出力フォーマット（デフォルト: `text`） |
| `--db <path>` | DB パスの上書き |
| `-y` / `--yes` | 確認プロンプトをスキップ（CI 用） |

## コマンド一覧

### `scan <project_dir>`

Content 配下の全 `.uasset` / `.umap` をスキャンして DB を更新する。

**重要**: scan コマンドは `.uasset` ファイル本体には一切触れない。DB レコードのみを操作する。

```
Options:
  --full-scan    mtime に関わらず全ファイルを強制再スキャン
  -y / --yes     DB クリーンアップの確認プロンプトをスキップ（CI 用）

Output (text):
  Scanning ./MyProject/Content... (1000 files)
    + 3 new assets indexed
    ~ 5 assets updated (mtime changed)
    ? 2 assets removed from disk

  The following DB records have no corresponding file on disk:
    /Game/Old/BP_Deprecated.uasset
    /Game/Temp/M_Test.uasset
  Remove these records from DB? [y/N]: y

  ✓ 998 assets total, 2 records cleaned, 2 skipped (parse error)

  Skipped:
    WARN Content/Broken/BP_X.uasset: invalid magic number
    WARN Content/Old/M_Y.uasset: unsupported version
```

`-y` フラグ使用時はプロンプトを出さずに自動削除する。

---

### `graph <project_dir>`

依存グラフの概要と循環依存を表示する。

```
Options:
  --cycles-only    循環依存のみ表示

Output (text):
  Dependency Graph Summary
    Total assets   : 998
    Total edges    : 4,231
    Circular deps  : 2 cycles detected

  Cycles:
    [1] BP_Player → BP_Enemy → BP_GameMode → BP_Player
    [2] M_Rock → MF_Shared → M_Rock
```

---

### `dead-assets <project_dir>`

どの Asset からも参照されていない Asset を一覧表示する。

```
Options:
  --type <AssetType>    型でフィルタ

Output (text):
  Unreferenced Assets (47 found)
    /Game/Unused/T_OldTexture.uasset          (Texture2D, 2.1 MB)
    /Game/Characters/SK_OldEnemy.uasset       (SkeletalMesh, 8.4 MB)
    ...
```

---

### `impact <asset_path>`

指定 Asset を削除・リネームした場合に壊れる Asset を列挙する。

`<asset_path>` はゲームパス（`/Game/...`）またはファイルシステムパスを受け付ける。

```
Output (text):
  Impact Analysis: /Game/Characters/BP_Player

  Direct referencing (3):
    /Game/Levels/L_Main.umap
    /Game/UI/WBP_HUD.uasset
    /Game/GameModes/BP_GameMode.uasset

  Transitive referencing (12):
    /Game/Levels/L_Tutorial.umap
    ... (9 more)

  Total impact: 12 assets
```

---

### `redirectors <project_dir>`

プロジェクト内の Redirector Asset を検出・列挙する。

**Phase 1 スコープ**: `ObjectRedirector` 型の Asset を検出して一覧表示するのみ。
redirect 先の解決（壊れた Redirector の判定）は Phase 2 以降で対応する。

```
Output (text):
  Redirectors (5 found)
    /Game/Characters/OldName.uasset
    /Game/Meshes/SM_OldRock.uasset
    /Game/Materials/M_Deprecated.uasset
    /Game/UI/WBP_OldWidget.uasset
    /Game/Blueprints/BP_OldEnemy.uasset

  Note: redirect target resolution is available in Phase 2 analysis.
```

---

### `find <project_dir> [options]`

DB を使った Asset 検索・フィルタリング。

```
Options:
  --type <AssetType>      型でフィルタ（例: Texture2D, Blueprint）
  --larger-than <bytes>   ファイルサイズ下限
  --smaller-than <bytes>  ファイルサイズ上限
  --unreferenced          参照されていない Asset のみ
  --path <pattern>        パスのパターンマッチ（glob）

Examples:
  uasset-lens find ./Project --type Texture2D --larger-than 4096
  uasset-lens find ./Project --unreferenced --type StaticMesh
  uasset-lens find ./Project --path "**/Characters/**"
```

---

## JSON 出力フォーマット（`--format json`）

`--format json` を指定した場合、各コマンドは以下の単一 JSON オブジェクト（または配列）を stdout に出力する。
エラー時は `exit 2` となり、stderr にエラーメッセージを出力する（JSON エラーエンベロープは設けない）。

### `scan`

```json
{
  "assets_total": 998,
  "new":          3,
  "updated":      5,
  "removed":      2,
  "skipped": [
    { "path": "Content/Broken/BP_X.uasset", "reason": "invalid magic number" }
  ]
}
```

### `graph`

```json
{
  "total_assets": 998,
  "total_edges":  4231,
  "cycles": [
    ["/Game/BP_Player", "/Game/BP_Enemy", "/Game/BP_GameMode", "/Game/BP_Player"],
    ["/Game/M_Rock", "/Game/MF_Shared", "/Game/M_Rock"]
  ]
}
```

### `dead-assets`

```json
[
  { "path": "/Game/Unused/T_OldTexture", "type": "Texture2D",    "file_size": 2097152 },
  { "path": "/Game/Characters/SK_OldEnemy", "type": "SkeletalMesh", "file_size": 8808038 }
]
```

### `impact`

```json
{
  "target":     "/Game/Characters/BP_Player",
  "direct":     ["/Game/Levels/L_Main", "/Game/UI/WBP_HUD", "/Game/GameModes/BP_GameMode"],
  "transitive": ["/Game/Levels/L_Tutorial"],
  "total":      4
}
```

### `redirectors`

```json
[
  { "path": "/Game/Characters/OldName",       "type": "ObjectRedirector" },
  { "path": "/Game/Meshes/SM_OldRock",        "type": "ObjectRedirector" }
]
```

### `find`

```json
[
  { "path": "/Game/Textures/T_Rock_D", "type": "Texture2D", "file_size": 4194304 },
  { "path": "/Game/Textures/T_Rock_N", "type": "Texture2D", "file_size": 2097152 }
]
```

---

## `.uasset-lens.toml` 設定ファイル（Phase 1 最小仕様）

プロジェクトルートに配置し、チームで git 管理する。存在しない場合はデフォルト設定で動作する。

### Phase 1 でサポートするフィールド

```toml
# .uasset-lens.toml

[scan]
# スキャン対象から除外するパス（content_root からの相対パス、前方一致）
exclude_paths = [
    "Content/Dev/",
    "Content/Test/",
    "Content/Developers/",
]
```

Phase 3 以降で命名規則・サイズバジェット等のフィールドを追加する。
