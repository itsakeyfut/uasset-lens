# `path` Command — Internal Design

## Execution Flow

```
1. Determine direction:
   └── input starts with '/' → game-to-file mode  (or --to-file flag)
   └── otherwise             → file-to-game mode
2. Locate content_root:
   └── --content-root provided → use as-is
   └── not provided → walk up from CWD looking for a 'Content/' directory
   └── error if not found (exit 2)
3. Apply conversion:
   └── file-to-game: strip content_root prefix + .uasset/.umap extension, prepend /Game/
   └── game-to-file: strip /Game/ prefix, append .uasset, prepend content_root
4. Print result to stdout
5. Return 0 (always, unless error)
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Path conversion logic | `uasset-lens-shared` (`AssetPath::from_fs_path`, `AssetPath::to_fs_path`) |
| Content root resolution | `uasset-lens-cli` |

## Content Root Auto-detection

The auto-detection walks up from the current working directory looking for a
`Content/` subdirectory. This allows using `path` from anywhere inside a UE project
without specifying `--content-root` explicitly:

```
/Projects/MyGame/Content/Characters/  → found at /Projects/MyGame/
```

`--content-root` is provided for scripts that run from arbitrary working directories
where auto-detection would fail.

## Conversion Rules

| Direction | Input | Output |
|---|---|---|
| file→game | `Content/Characters/BP_Player.uasset` | `/Game/Characters/BP_Player` |
| game→file | `/Game/Characters/BP_Player` | `Content/Characters/BP_Player.uasset` |

The `.uasset` extension is assumed for game→file conversions. `.umap` files must
be converted via their filesystem path (file→game direction).

## JSON Output

```json
{ "input": "Content/Characters/BP_Player.uasset", "output": "/Game/Characters/BP_Player" }
```

Both `input` and `output` fields are always present regardless of conversion direction.
