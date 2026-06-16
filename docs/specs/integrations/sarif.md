# SARIF Output Format — Specification

## Purpose

When `--format sarif` is passed to a supported command, `uasset-lens` outputs a SARIF
2.1.0 document suitable for upload to GitHub Advanced Security (GHAS) or any other
SARIF-consuming tool.

Reference: [SARIF 2.1.0 specification](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html)

---

## Supported Commands

| Command | SARIF support |
|---|---|
| `check` | yes |
| `lint` | yes |
| `budget` | yes |
| `scan` | no |
| `find` | no |
| `stats` | no |
| `graph` | no |
| `impact` | no |

Commands that do not support SARIF exit with code `2` and print an error to stderr if
`--format sarif` is passed.

---

## Document Structure

### Top-level fields

| Field | Value |
|---|---|
| `$schema` | `https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0-rtm.5.json` |
| `version` | `"2.1.0"` |
| `runs` | Array with exactly one run object |

### `runs[0].tool.driver`

| Field | Value |
|---|---|
| `name` | `"uasset-lens"` |
| `version` | Current crate version (e.g., `"0.2.0"`) |
| `informationUri` | `"https://github.com/itsakeyfut/uasset-lens"` |
| `rules` | One entry per lint/budget rule ID that produced at least one result |

### `runs[0].results[]`

| Field | Description |
|---|---|
| `ruleId` | Rule identifier (see Rule ID Format below) |
| `level` | `"error"` or `"warning"` (see Level Mapping below) |
| `message.text` | Human-readable violation message |
| `locations[0].physicalLocation.artifactLocation.uri` | Filesystem path relative to repo root, forward-slash separated |
| `locations[0].physicalLocation.artifactLocation.uriBaseId` | `"%SRCROOT%"` |

---

## Rule ID Format

Rule IDs use a slash-separated hierarchy:

| Pattern | Example |
|---|---|
| `lint/<category>/<rule-name>` | `lint/naming/blueprint-prefix` |
| `budget/<asset-type>` | `budget/texture2d` |
| `dead-asset` | `dead-asset` |
| `duplicate/<criterion>` | `duplicate/same-name` |

---

## Level Mapping

| Finding type | SARIF level |
|---|---|
| Lint error | `"error"` |
| Lint warning | `"warning"` |
| Budget violation | `"error"` |
| Dead asset | `"warning"` |
| Duplicate asset | `"warning"` |

---

## Example Output

```json
{
  "$schema": "https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0-rtm.5.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "uasset-lens",
          "version": "0.2.0",
          "informationUri": "https://github.com/itsakeyfut/uasset-lens",
          "rules": [
            {
              "id": "lint/naming/blueprint-prefix",
              "name": "BlueprintPrefixRule",
              "shortDescription": {
                "text": "Blueprint assets must use BP_ prefix"
              }
            },
            {
              "id": "budget/texture2d",
              "name": "Texture2DBudgetRule",
              "shortDescription": {
                "text": "Texture2D asset exceeds configured size budget"
              }
            }
          ]
        }
      },
      "results": [
        {
          "ruleId": "lint/naming/blueprint-prefix",
          "level": "error",
          "message": {
            "text": "Asset 'Character' is missing required prefix 'BP_'"
          },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": {
                  "uri": "Content/Characters/Character.uasset",
                  "uriBaseId": "%SRCROOT%"
                }
              }
            }
          ]
        },
        {
          "ruleId": "budget/texture2d",
          "level": "error",
          "message": {
            "text": "Texture2D 'T_RockAlbedo' is 8.2 MB; budget is 4.0 MB"
          },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": {
                  "uri": "Content/Environment/Rocks/T_RockAlbedo.uasset",
                  "uriBaseId": "%SRCROOT%"
                }
              }
            }
          ]
        }
      ]
    }
  ]
}
```

---

## GitHub Advanced Security Upload

After generating the SARIF file, upload it with the `upload-sarif` action:

```yaml
- name: Run uasset-lens
  run: |
    uasset-lens scan ./MyProject
    uasset-lens check ./MyProject --format sarif > results.sarif
  continue-on-error: true

- name: Upload SARIF to GitHub
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

`continue-on-error: true` ensures the upload step runs even when violations are found.

---

## Encoding Notes

- All paths in `artifactLocation.uri` use forward slashes regardless of the host OS.
- The SARIF document is written as UTF-8 without BOM.
- When no violations are found, `results` is an empty array (`[]`) and `rules` is an
  empty array (`[]`). A valid zero-violation SARIF document is still produced.
