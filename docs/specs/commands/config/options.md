# `config` Command — Options

## Synopsis

```
uasset-lens config <subcommand> [options]
```

Available subcommands:

| Subcommand | Description |
|---|---|
| `validate` | Validate the config file and report errors |

---

## `validate` Subcommand

### Synopsis

```
uasset-lens config validate <project_dir> [options]
```

---

### Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root containing `.uasset-lens.toml` |

---

### Options

#### `--config <path>`

Validate a specific config file instead of the default `.uasset-lens.toml` in
`<project_dir>`.

Useful for validating a config template or a config from a different environment
before copying it into place.

```bash
uasset-lens config validate ./Project --config ./configs/aaa.toml
uasset-lens config validate ./Project --config /shared/team.uasset-lens.toml
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json>` | | Output format (default: `text`) |
