# `path` Command — Options

## Synopsis

```
uasset-lens path <input> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<input>` | Yes | The path to convert — filesystem path or `/Game/...` game path |

---

## Options

### `--to-file`

Convert a game path to a filesystem path.

Without this flag, a game path is still auto-detected if the input starts with `/Game/`.
This flag is useful for explicitness in scripts.

```bash
uasset-lens path /Game/Characters/BP_Player --to-file
# Output: Content/Characters/BP_Player.uasset
```

---

### `--content-root <PATH>`

Specify the Content root directory explicitly. Auto-detected if not provided.

```bash
uasset-lens path /Game/BP_Player --to-file --content-root /Projects/MyGame/Content
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--yes` | `-y` | Skip confirmation prompts |

Note: `--db` and `--config` are accepted globally but have no effect for this command,
as `path` does not use the database or config file.
