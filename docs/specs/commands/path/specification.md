# `path` Command — Specification

## Purpose

Convert between filesystem paths and UE game paths (`/Game/...`). A utility for
scripting and automation when you need to translate between the two path conventions.

```bash
uasset-lens path Content/Characters/BP_Player.uasset
uasset-lens path /Game/Characters/BP_Player --to-file
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Always (unless execution error) |
| `2` | Execution error (content root not found, invalid path) |

---

## Auto-detection

If the input starts with `/Game/`, it is treated as a game path and converted to a
filesystem path (equivalent to `--to-file`). Otherwise it is treated as a filesystem
path and converted to a game path.

---

## Text Output

```
# Filesystem path → game path (auto-detected)
$ uasset-lens path Content/Characters/BP_Player.uasset
/Game/Characters/BP_Player

# Game path → filesystem path (explicit)
$ uasset-lens path /Game/Characters/BP_Player --to-file
Content/Characters/BP_Player.uasset
```

---

## Content Root Resolution

The content root is auto-detected by walking up from the current directory to find
a `Content/` directory. Use `--content-root` to specify it explicitly if auto-detection
fails (e.g., in scripts that run from an arbitrary working directory).

```bash
uasset-lens path /Game/Characters/BP_Player --to-file --content-root /Projects/MyGame/Content
```

---

## JSON Output (`--format json`)

```json
{ "input": "Content/Characters/BP_Player.uasset", "output": "/Game/Characters/BP_Player" }
```
