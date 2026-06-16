# `impact` Command — Options

## Synopsis

```
uasset-lens impact <project_dir> <asset_path> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |
| `<asset_path>` | Yes | Target asset — UE game path (`/Game/...`) or filesystem path |

---

## Options

### `--tree`

Show the full propagation tree instead of flat direct/transitive lists.

By default, the output shows two flat lists: "Direct referencing" and "Transitive
referencing". With `--tree`, each referencing asset's own referencing chain is shown
as a tree, making it clear how impact propagates through the project.

```bash
uasset-lens impact ./Project /Game/Characters/BP_Player --tree
```

---

### `--shortest-path <TARGET>`

Find the shortest dependency chain between `<asset_path>` and `<TARGET>`.

Prints the fewest-hop path from `<asset_path>` to `<TARGET>`, showing exactly how
the target depends on the source through the chain.

```bash
uasset-lens impact ./Project /Game/Materials/M_Rock --shortest-path /Game/Levels/L_Main
```

Output:
```
Shortest path from /Game/Materials/M_Rock to /Game/Levels/L_Main (3 hops):
  /Game/Materials/M_Rock
  → /Game/Meshes/SM_Rock   (StaticMesh)
  → /Game/Characters/BP_Goblin (Blueprint)
  → /Game/Levels/L_Main    (World)
```

Exits `1` if no path exists between the two assets.

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
