# `impact` Command — Internal Design

## Execution Flow

```
1. resolve_asset_path(project_dir, asset_path)  [cli]
2. AssetDb::open(db_path)                       [asset-db]
3. load_graph(&db, external_roots)              [dependency-graph]
4. Validate: graph.contains(&target) → error if not found
5. impact_analyzer::detect(&graph, &target)     [impact-analyzer]
   └── BFS on reversed edges
   └── returns ImpactResult { direct: Vec<AssetPath>, transitive: Vec<AssetPath> }
6. total = direct.len() + transitive.len()
7. Format output:
   └── flat mode (default): list direct then transitive (cap transitive at 10 in text)
   └── --tree: recursive DFS with TreeNode building
8. Return: 1 if total > 0, 0 if no impact
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Reverse edge traversal | `uasset-lens-impact-analyzer` |
| Graph structure | `uasset-lens-dependency-graph` |
| Tree rendering | `uasset-lens-cli` |

## Direct vs Transitive Separation

`impact_analyzer::detect()` performs a BFS on the reversed dependency graph:

```
target node → find all nodes N where N → target (direct, depth 1)
           → find all nodes M where M →+ target (transitive, depth 2+)
```

A node is **direct** if it references the target in a single hop.
A node is **transitive** if it reaches the target only through one or more intermediate nodes.

The same node cannot be both direct and transitive — once classified at depth 1,
it is not re-visited.

## Tree Mode (`--tree`)

The tree is built by `build_tree()` using a recursive DFS with a `path_stack: HashSet<AssetPath>`:

```rust
fn build_tree(graph, path, depth, path_stack) -> TreeNode:
    if path_stack.contains(path):
        return TreeNode { kind: Cycle, children: [] }  // stop recursion
    kind = if depth == 1 { Direct } else { Transitive }
    path_stack.insert(path)
    children = graph.reverse_deps_of(path)
                    .map(|p| build_tree(graph, p, depth+1, path_stack))
    path_stack.remove(path)           // allow path to appear in sibling branches
    TreeNode { path, kind, children }
```

The `path_stack` is thread-local to the current DFS branch (removed on backtrack),
which prevents false cycle detection across sibling subtrees while correctly detecting
true cycles within a path.

## NodeKind Classification

```rust
enum NodeKind {
    Direct,     // depth == 1 from target
    Transitive, // depth >= 2
    Cycle,      // node already in path_stack (would recurse infinitely)
}
```

Text output labels: `(direct)`, `(via ParentName)`, `[cycle]`.

## Transitive Cap (Text Mode)

In flat text mode, transitive results are capped at 10 entries with a `... (N more)` line.
JSON and tree modes have no cap.
