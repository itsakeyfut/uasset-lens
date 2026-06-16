# `blueprint` Command — Options

## Synopsis

```
uasset-lens blueprint <project_dir>
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--inheritance <ASSET>`

Show the full inheritance tree rooted at the given Blueprint class.

Prints all Blueprint subclasses (direct and transitive) that inherit from `<ASSET>`.
Useful for understanding the class hierarchy and the scope of a base class change.

```bash
uasset-lens blueprint ./Project --inheritance /Game/Characters/BP_BaseCharacter
```

Output:
```
Inheritance tree for BP_BaseCharacter (Blueprint)

BP_BaseCharacter
├─ BP_Player           (/Game/Characters/BP_Player)
├─ BP_Enemy            (/Game/Characters/Enemies/BP_Enemy)
│  ├─ BP_Goblin        (/Game/Characters/Enemies/BP_Goblin)
│  └─ BP_Orc           (/Game/Characters/Enemies/BP_Orc)
└─ BP_NPC              (/Game/Characters/BP_NPC)

5 subclasses (2 direct, 3 transitive)
```

---

### `--coupling`

Show Blueprint coupling metrics: how many other assets each Blueprint depends on
and is depended on by.

High coupling (many deps + high in-degree) indicates a potential architectural hotspot.

```bash
uasset-lens blueprint ./Project --coupling
```

Output (top 10 by coupling score):
```
Blueprint Coupling (top 10)

Asset                                  Out  In  Score
/Game/GameModes/BP_GameMode           45   8   53
/Game/Characters/BP_BaseCharacter     32   12  44
/Game/Core/BP_GameInstance            28   15  43
...

Score = out_degree + in_degree. Higher = more coupled.
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
