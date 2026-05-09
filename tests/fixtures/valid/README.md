# Valid Fixtures

Real `.uasset` / `.umap` files copied from the **Warrior** UE project.
Used in scanner integration tests to verify correct parsing of well-formed assets.

## Source project

| Item | Value |
|------|-------|
| Project | Warrior |
| UE version | 5.4 |
| Content root | `<Project>/Content/` |

## Fixture catalogue

| File | Asset Type | Original filename | Size | Notes |
|------|-----------|-------------------|------|-------|
| `BP_Simple.uasset` | Blueprint | `GameModes/BP_BaseGameMode.uasset` | 21 KB | Game mode blueprint; contains `/Game/` imports |
| `L_TestMap.umap` | World | `Maps/FeatureDevMap.umap` | 21 KB | Feature development level |
| `M_Basic.uasset` | Material | `Assets/Niagara/GroundRocks/M_RockBase.uasset` | 22 KB | Base rock material |
| `SM_Cube.uasset` | StaticMesh | `Assets/Enemies/Enemy_Troll/Meshes/SM_Troll_hammer.uasset` | 52 KB | Troll weapon mesh |
| `T_Rock.uasset` | Texture2D | `Assets/Enemies/Enemy_Troll/Textures/T_Troll_D.uasset` | 2.8 MB | Diffuse texture (large due to texture data) |
| `Redirect.uasset` | ObjectRedirector | `GameModes/BP_BaseGameMode.uasset` (redirector stub) | 2.5 KB | Forwarding stub left after renaming `BP_BaseGameMode` |

## How to regenerate

1. Open the Warrior project in the UE editor (same version as above).
2. Copy each file from `<Project>/Content/<Original filename>` into this directory.
3. Rename to match the fixture filename in the table above.

## Generating `Redirect.uasset`

ObjectRedirector stubs are created automatically when an asset is renamed or moved in the Content Browser (UE 5.4: no dialog — stub is created silently):

1. Open the Warrior project in the UE editor.
2. Rename or move a small asset (e.g. `BP_BaseGameMode`).
3. The stub left at the original location has a → arrow icon and is ~2 KB.
4. Copy it here as `Redirect.uasset`.
