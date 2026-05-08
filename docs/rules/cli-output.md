# uasset-lens — CLI Output Rules

## stdout と stderr の分離

| ストリーム | 用途 |
|-----------|------|
| `stdout` | コマンドの結果出力（テキスト・JSON） |
| `stderr` | エラーメッセージ・警告・進捗インジケーター |

`stdout` をパイプで渡したときに進捗ノイズが混入しないようにするため。

```bash
# ✅ この使い方が正しく機能する
uasset-lens impact ./Project/Content/BP_Player.uasset --format json | jq '.direct[]'
```

---

## stdout / stderr の実装ルール

### ライブラリクレートは stdout/stderr に書かない

`scanner`・`asset-db` などのライブラリクレートで `println!` / `eprintln!` を使うことを禁止する。
画面出力は `cli` クレートのみ行う。ライブラリはログ（`tracing`）のみ使用する。

### 進捗表示は stderr へ

```rust
// ✅ Progress to stderr
eprintln!("Scanning {} files...", count);

// ✅ Result to stdout
println!("  {}", asset_path);
```

---

## テキスト出力フォーマット

### 件数サマリーは最後に出力する

```
  /Game/Unused/T_OldTexture
  /Game/Characters/SK_OldEnemy
  ...

  Unreferenced Assets (47 found)
```

### ゼロ件でも出力する

```
  Dead Assets (0 found)
```

### エラーメッセージは `Error:` プレフィックスを付けて stderr へ

```rust
// ✅
eprintln!("Error: no scan data found.");
eprintln!("Run 'uasset-lens scan <project_dir>' first.");
```

---

## JSON 出力（`--format json`）

- `stdout` に**単一の JSON 値**（オブジェクトまたは配列）を出力する
- エンベロープ（`{ "ok": true, "data": ... }` 形式）は使わない
- ANSI カラーコードを JSON 出力に含めない
- エラー時は `stderr` にエラーメッセージを出力し `exit 2` する（JSON エラーオブジェクトは出力しない）
- `serde_json::to_string_pretty` で整形して出力する

```rust
// ✅ JSON output
if opts.format == OutputFormat::Json {
    let json = serde_json::to_string_pretty(&result)
        .context("Failed to serialize output")?;
    println!("{json}");
    return Ok(());
}
```

各コマンドの JSON スキーマは `docs/specs/cli-design.md` の「JSON 出力フォーマット」セクションを参照。

---

## ANSI カラー

- stdout が端末（`IsTerminal`）のときのみ ANSI カラーコードを使う
- `NO_COLOR` 環境変数が設定されている場合はカラーを無効にする
- JSON 出力時はカラーを使わない
- CI 環境では通常カラーが無効になる（`NO_COLOR` または非 TTY）

```rust
// ✅ Terminal check before color
use std::io::IsTerminal;

let use_color = std::io::stdout().is_terminal()
    && std::env::var("NO_COLOR").is_err();
```

---

## Exit Codes

| コード | 意味 |
|--------|------|
| `0` | 正常終了・問題なし |
| `1` | 問題を検出（dead asset・循環依存・impact あり 等） |
| `2` | 実行エラー（IO エラー・DB 未作成・パース失敗 等） |

```rust
// ✅ Exit code via process::exit or via main() return value
fn main() -> ExitCode {
    match run() {
        Ok(IssuesFound::None)  => ExitCode::SUCCESS,       // 0
        Ok(IssuesFound::Some)  => ExitCode::from(1),       // 1
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(2)                              // 2
        }
    }
}
```

---

## scan コマンドの削除確認プロンプト

ディスク上から消えた Asset の DB レコードを削除する前に必ずユーザーに確認を求める。
`-y` / `--yes` フラグが指定されている場合のみ自動削除する。

```
The following DB records have no corresponding file on disk:
  /Game/Old/BP_Deprecated
  /Game/Temp/M_Test
Remove these 2 records from DB? [y/N]:
```

```rust
// ✅ Prompt unless -y
if !opts.yes {
    eprint!("Remove {} records from DB? [y/N]: ", stale.len());
    let mut ans = String::new();
    std::io::stdin().read_line(&mut ans)?;
    if !ans.trim().eq_ignore_ascii_case("y") {
        return Ok(());
    }
}
```

**重要**: このコマンドは DB レコードを削除するのみ。実際の `.uasset` ファイルには一切触れない。
