# `graph` Command — Options

## Synopsis

```
uasset-lens graph <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--cycles-only`

Print only circular dependencies; suppress the graph summary header.

Exits `1` if any cycles are found — useful as a CI gate.

```bash
uasset-lens graph ./Project --cycles-only
```

---

### `--full-cycles`

Show all nodes in long cycles instead of collapsing intermediate nodes.

By default, cycles with many nodes are shown as `A → B → ... (N more) → A`.
With `--full-cycles`, every node in the cycle is printed.

```bash
uasset-lens graph ./Project --full-cycles
uasset-lens graph ./Project --cycles-only --full-cycles
```

---

### `--soft-refs`

Include soft object references (`FSoftObjectPath`) in the graph in addition to
hard import-table references.

Soft references are extracted as a separate edge type and stored in the
`soft_dependencies` table. See `docs/specs/analyzers/soft-ref-cycles.md`.

```bash
uasset-lens graph ./Project --soft-refs --cycles-only
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|dot>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |

When `--format dot` is used, the full dependency graph is exported in Graphviz DOT
format. For large projects (>10k assets) the output can be very large; consider
combining with a filter flag or using `deps` for a subgraph.

```bash
uasset-lens graph ./Project --format dot > full-graph.dot
uasset-lens graph ./Project --cycles-only --format dot > cycles.dot
```
