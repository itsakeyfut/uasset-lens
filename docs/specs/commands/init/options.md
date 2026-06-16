# `init` Command — Options

## Synopsis

```
uasset-lens init <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root where `.uasset-lens.toml` will be written |

---

## Options

### `--preset <indie|mid|aaa>`

Skip the interactive prompt and apply the specified preset directly.

| Value | Target scale |
|---|---|
| `indie` | Small projects with fewer than 1,000 assets |
| `mid` | Medium projects with 1,000–10,000 assets |
| `aaa` | Large projects with more than 10,000 assets |

```bash
uasset-lens init ./Project --preset mid
uasset-lens init ./Project --preset aaa
```

---

### `--force`

Overwrite an existing `.uasset-lens.toml` without prompting.

Without this flag, `init` exits with code `1` if the config file already exists.

```bash
uasset-lens init ./Project --preset indie --force
```

---

### `-y` / `--yes`

Assume defaults for all prompts and skip all interactive confirmations.

Equivalent to pressing Enter at every interactive prompt. When combined with `--preset`,
writes the file immediately without any user interaction.

```bash
uasset-lens init ./Project -y
uasset-lens init ./Project --preset mid --yes
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json>` | | Output format (default: `text`) |
| `--yes` | `-y` | Skip confirmation prompts |
