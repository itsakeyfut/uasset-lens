# `crates/scanner` — Export property parser (`parser/properties.rs`)

## Summary

Implement binary parsing of UE5 Export object property data (`FProperty` / `FTag`)
to enable Blueprint metrics extraction in Issue #2.
Complete when a Blueprint fixture's property stream is parsed without errors and
yields a structured property list.

## Design Notes

**Context:** Phase 1 parsed only the table headers (NameTable, ImportTable, ExportTable).
To extract Blueprint metrics (node count, EventTick usage, etc.), we need to parse
the actual **serialized property data** of Export objects.

**UE5 property serialization format (tagged properties):**

```
loop:
  PropertyName  FString    — name table index; "None" signals end of properties
  if PropertyName == "None": break

  PropertyType  FString    — e.g. "IntProperty", "BoolProperty", "ObjectProperty", "ArrayProperty"
  PropertySize  i64        — byte size of the value (used to skip unknown types)
  ArrayIndex    i32        — usually 0
  Tag           bytes      — type-specific tag (varies by PropertyType)
  Value         bytes      — PropertySize bytes of value data
```

**Strategy for Phase 4:**
- Parse the property stream until `PropertyName == "None"`
- For each property, read `PropertyType` and decide:
  - Known types needed for Blueprint metrics: read the value
  - Unknown types: skip exactly `PropertySize` bytes (never return an error for unknown types)
- Return `Vec<ParsedProperty>` where `ParsedProperty` is an enum of the types we care about

**Types to handle (minimum for Blueprint metrics):**
- `IntProperty` → `i32` value
- `BoolProperty` → `bool` value
- `ArrayProperty` → item count (don't need full recursion for Phase 4)
- `ObjectProperty` → name index (reference to import/export table entry)

## Requirements

- [ ] Create `parser/properties.rs` in `crates/scanner`
- [ ] Implement `parse_properties(data: &[u8], offset: u64, name_table: &[String]) -> Result<Vec<ParsedProperty>, ScanError>`
- [ ] Define `ParsedProperty` enum: `Int { name: String, value: i32 }`, `Bool { name: String, value: bool }`, `Array { name: String, count: usize }`, `Object { name: String, class_name: String }`, `Skipped { name: String }`
- [ ] Parse property loop: stop on `"None"` name
- [ ] Handle unknown `PropertyType` by advancing exactly `PropertySize` bytes (no error)
- [ ] Unit test: byte sequence for 2–3 known properties followed by `"None"` → correct ParsedProperty list
- [ ] Unit test: unknown property type is skipped, parsing continues for subsequent properties

## Related

- Next: #2 — Blueprint Export property extraction
- Docs: `docs/roadmap/phase4/ROADMAP.md` — technical prerequisite section, `docs/rules/binary-parser.md`
