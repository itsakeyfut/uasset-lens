# Niagara Analysis — Specification

## Purpose

Analyze `NiagaraSystem` assets to extract emitter count and GPU simulation / dynamic light
flags. Enables lint rules that catch particle systems likely to cause GPU frame budget
spikes.

---

## Scope

| Asset Class | Metadata Extracted | Notes |
|---|---|---|
| `NiagaraSystem` | Yes — full metadata | Primary target |
| `NiagaraEmitter` | No new metadata | Referenced as dependencies from `NiagaraSystem` |
| `NiagaraScript` | No new metadata | Referenced as dependencies from emitters |

---

## Metadata Extracted (Scanner Additions)

All fields are read from the `NiagaraSystem` export property stream.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `emitter_count` | `int32` | `Emitters` array length in export properties | Number of emitter handles embedded in the system |
| `has_gpu_simulation` | `bool` | Any emitter's `SimTarget = GPUComputeSim` | True if at least one emitter uses GPU simulation |
| `has_lights` | `bool` | Any emitter renderer of type `NiagaraLightRendererProperties` | True if at least one emitter spawns dynamic lights |

Fields absent from the binary are stored as `NULL`.

---

## Database Schema

```sql
CREATE TABLE niagara_metadata (
    asset_path       TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    emitter_count    INTEGER,
    has_gpu_simulation INTEGER,  -- 0 or 1
    has_lights         INTEGER   -- 0 or 1
);
```

---

## Dependency Extraction

Niagara assets reference the following asset types via their import table. These are captured
by the existing dependency extraction pass — no special parser extension is needed:

| Referenced Type | How Referenced |
|---|---|
| `NiagaraEmitter` | Embedded emitter handles in `NiagaraSystem.Emitters` |
| `NiagaraScript` | Emitter and system scripts (update/spawn/event) |
| `StaticMesh` | Mesh renderer properties |
| `Material` | All renderer types reference a material |
| `Texture2D` | Sprite and ribbon renderers reference textures directly |

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/niagara/many-emitters` | Warning | `emitter_count > 10` | Each emitter adds CPU tick cost and potentially separate GPU dispatch overhead; large counts indicate a system that should be split or consolidated |
| `lint/niagara/gpu-sim-with-lights` | Warning | `has_gpu_simulation = 1` and `has_lights = 1` | GPU simulated particles with dynamic lights require readback from the GPU to place lights on the CPU, stalling the render thread |

---

## Budget Rules

```toml
[budget]
NiagaraSystem = "10MB"  # per-file default; configurable in .uasset-lens.toml
```

---

## UE5 Binary Format Notes

`NiagaraSystem` stores emitters as `FNiagaraEmitterHandle` array elements under the
`Emitters` property (`ArrayProperty` of `StructProperty`). Each handle contains a
`NiagaraEmitter` sub-object reference and a `bIsEnabled` flag.

Simulation target (`SimTarget`) is a `ByteProperty` enum inside `UNiagaraEmitter`'s
serialized properties; value `1` corresponds to `GPUComputeSim`.

Light renderer presence is detected by scanning each emitter's `RendererProperties` array
for elements whose class name contains `NiagaraLightRendererProperties`.

These inner emitter properties are available in editor `.uasset` files. In cooked builds
some emitter details may be stripped; treat stripped fields as `NULL` rather than `false`.
