# Sound Analysis — Specification

## Purpose

Analyze `SoundWave`, `SoundCue`, `MetaSoundSource`, and `MetaSoundPatch` assets to extract
audio quality settings, duration, and streaming configuration. Enables lint rules that catch
over-sized or misconfigured audio assets.

---

## Scope

| Asset Class | Metadata Extracted | Notes |
|---|---|---|
| `SoundWave` | Yes — full metadata | Primary target |
| `SoundCue` | No new metadata | Dependencies captured via import table |
| `MetaSoundSource` | No new metadata | Dependencies captured via import table |
| `MetaSoundPatch` | No new metadata | Dependencies captured via import table |

---

## Metadata Extracted (Scanner Additions)

All fields are read from the `SoundWave` export property stream.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `duration` | `float` | Export property `Duration` | Audio length in seconds |
| `num_channels` | `int32` | Export property `NumChannels` | 1 = mono, 2 = stereo |
| `sample_rate` | `int32` | Export property `SampleRate` | Samples per second, e.g. 44100, 48000 |
| `compression_quality` | `int32` | Export property `CompressionQuality` | 0–100; 100 = uncompressed |
| `streaming_priority` | `int32` | Export property `StreamingPriority` | Higher values load sooner |
| `is_streaming` | `bool` | Export property `bStreaming` | Whether audio streams from disk at runtime |

Fields absent from the binary (not serialized, using engine defaults) are stored as `NULL`.

---

## Database Schema

```sql
CREATE TABLE sound_metadata (
    asset_path          TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    duration_secs       REAL,
    num_channels        INTEGER,
    sample_rate         INTEGER,
    compression_quality INTEGER,
    streaming_priority  INTEGER,
    is_streaming        INTEGER  -- 0 or 1; NULL if unknown
);
```

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/sound/stereo-ambient` | Warning | `duration > 10` and `num_channels = 2` | 3D positional audio is mono-mixed at runtime; stereo ambient doubles streaming bandwidth with no audible benefit |
| `lint/sound/uncompressed` | Error | `compression_quality = 100` and file size `> 2MB` | Uncompressed audio above 2 MB occupies excessive memory and streaming bandwidth |
| `lint/sound/high-sample-rate` | Warning | `sample_rate > 48000` | Sample rates above 48 kHz are inaudible on consumer hardware and waste storage |
| `lint/sound/non-streaming-large` | Warning | `duration > 30` and `is_streaming = 0` | Sounds longer than 30 seconds loaded entirely into memory risk budget overruns at runtime |

---

## Budget Rules

```toml
[budget]
SoundWave = "4MB"   # per-file default; configurable in .uasset-lens.toml
```

---

## MetaSound / SoundCue Dependency Extraction

`MetaSoundSource` and `MetaSoundPatch` reference other MetaSound patches and `SoundWave`
nodes via an internal node graph. These are extracted as standard hard dependencies because
MetaSound node inputs are serialized as object references in the asset's import table.

`SoundCue` references `SoundWave` (and other cues) via its node tree, which is also
represented in the import table.

No special parser extension is required for MVP. If a reference appears in the import table
it is captured by the existing dependency extraction pass.

---

## UE5 Binary Format Notes

`SoundWave` export properties are serialized using the standard tagged property format.
`Duration` is a `FloatProperty`; `NumChannels` and `SampleRate` are `IntProperty`.
`bStreaming` is a `BoolProperty`. `CompressionQuality` is an `IntProperty` ranging 0–100.

All fields appear in the property stream before the `None` terminator. Order is not
guaranteed — the parser must read by tag name, not position.
