# SPEC.md — skillctl specification (MVP → v1)

## 1. Overview

`skillctl` is a CLI that **copies and synchronizes** agent skills from a
**global (canonical)** store to user-defined **targets (name + root directory)**.

* **Unit of management**: **directory** at `<root>/<skill_id>/...`
* **Identity check**: directory digest based on **relative path + content**
* **Diff inspection**: run an external diff tool
* **Safety**: `--dry-run` only (complete plan listing, zero file operations)

---

## 2. Goals / non-goals

### 2.1 Goals

* Visualize **diff states** between global and target
* Converge from global to target (**install/update/skip**)
* Import from target to global when needed (default: add-only)
* Compare a diffing skill via **diff tool**

### 2.2 Non-goals (out of scope for MVP)

* Conflict resolution such as 3-way merge
* Automatic backups or interactive confirmations
* Remote fetch (e.g. direct install from GitHub)
* Watch/daemon/autosync

---

## 3. Terms

* **global_root**: canonical skills directory
* **target**: sync destination (user-defined name + root path)
* **skill_id**: directory name (e.g. `git-release`). No separators, `..`, or absolute paths
* **digest**: hash computed from relative paths + contents in a directory
* **state**: comparison result between global and target (missing/same/diff/extra)

---

## 4. Directory model

### 4.1 Global (canonical)

* `global_root/<skill_id>/...`
* `skill_id` must be a **normal directory** (no symlinks)

### 4.2 Target

* `targets[].root/<skill_id>/...`
* `targets[].root/<skill_id>` must be a **normal directory** (no symlinks)

Codex/OpenCode-specific discovery paths are **not** assumed by this tool
(targets are fully user-defined).

---

## 5. Configuration (`config.toml`)

### 5.1 Default path

Priority order:

1. Use `SKILLCTL_CONFIG` if set
2. If `XDG_CONFIG_HOME` is set, `${XDG_CONFIG_HOME}/skillctl/config.toml`
3. Otherwise `~/.config/skillctl/config.toml`

### 5.2 Required schema

* `global_root: string`
* `targets: array`

  * `name: string` (unique)
  * `root: string`

### 5.3 Optional schema

* `[hash]`

  * `algo: "blake3" | "sha256"` (default: `blake3`)
  * `ignore: string[]` (glob patterns, default: empty)
* `[diff]`

  * `command: string[]` (argv form, default: `git diff --no-index -- {left} {right}`)

### 5.4 Path expansion

* Expand `~` and environment variables (`$VAR` / `${VAR}`)

### 5.5 Message language

* If `SKILLCTL_LANG` is set, choose `ja` / `en`
* Otherwise check `LC_ALL` / `LC_MESSAGES` / `LANG`
* Unsupported values default to `ja`

---

## 6. Digest specification (relative path + content)

### 6.1 Scope

* **Regular files** under a skill directory
* **Relative path** (normalized per stabilization rules)
* **Content** (raw bytes)
* **Symlinks are not supported** (error)

### 6.2 Excluded from hash

* Metadata such as mtime, owner, permissions
* Directory enumeration order (order is normalized)

### 6.3 Stabilization rules

1. Enumerate files after applying `ignore`
2. Sort by **relative path ascending**
3. Feed **relative path + content** into the hash (renames are diffs)

### 6.4 ignore

* Files matching `hash.ignore` globs are excluded
* Recommended defaults (example): `.git/**`, `**/.DS_Store`, `**/*.tmp`

---

## 7. State determination (`status`)

### 7.1 States

* `missing`: exists in global, not in target
* `same`: exists in both, digest matches
* `diff`: exists in both, digest differs
* `extra`: exists only in target (not in global)

### 7.2 Output (default: table)

* Columns: `SKILL | STATE | GLOBAL_DIGEST | TARGET_DIGEST`
* Digest may be shortened (e.g. first 3 + last 3)

---

## 8. Sync specification (`push` / `import`)

`push` and `import` must separate **Plan (diff/ops)** and **Execute**.

### 8.1 push (global → target)

* Input: `<skill_id>` or `--all`, `--target <name>`
* Decisions:

  * `missing` → **install**
  * `diff` → **update**
  * `same` → **skip**
* Update method (implementation requirement):

  * Copy to a temp location, then replace (no partial state)
* `--dry-run`:

  * List install/update/skip (+ prune if applicable)
  * No file operations

#### `--prune` (optional)

* Include target-only skills (`extra`) for removal
* Default is not to prune (safer)

### 8.2 import (target → global)

* Input: `<skill_id>` or `--all`, `--from <name>`
* Default behavior:

  * Import only skills missing in global (**install**)
* `--overwrite`:

  * Replace global if same-name exists (explicit only)
* `--dry-run`:

  * List planned ops, no file operations

---

## 9. diff specification

* `diff.command` is argv and must include both `{left}` and `{right}` at least once
* If either placeholder is missing, return a config error (exit code 3)
* `skillctl diff <skill> --target <name>` replaces placeholders and runs it
* If either path is missing, return an error with next action guidance
* Diff exit codes: treat **0/1 as success**, others as error

---

## 10. doctor specification

* `doctor --global | --target <name> | --all`
* Checks per skill directory:
  * `SKILL.md` exists and is a **regular file** (not symlink)
  * No **symlinks** inside the skill directory
  * No **unsupported file types** (only dirs/files)
* Output format (per root):
  * `ok <skill>` when no issues
  * `issue <skill> <message>` for each issue
  * Summary: `checked: <count> issues: <count>`
* When `--all` is specified, outputs a labeled section per target

---

## 11. CLI commands (MVP)

### 11.1 Command list

* `targets`
* `list --global | --target <name>`
* `status --target <name> | --all`
* `doctor --global | --target <name> | --all`
* `push [<skill>|--all] --target <name> [--dry-run] [--prune]`
* `import [<skill>|--all] --from <name> [--dry-run] [--overwrite]`
* `diff <skill> --target <name>`

### 11.2 Exit codes

* `0`: success
* `2`: invalid CLI arguments
* `3`: config errors (missing/invalid config, unknown target, etc.)
* `4`: execution errors (copy failure, diff launch failure, etc.)

---

## 12. Acceptance criteria (MVP)

* `status` outputs all four states correctly
* `push --dry-run` lists planned ops and makes zero file changes
* After `push`, target skills converge to `same`
* `import` defaults to add-only, `--overwrite` replaces
* `diff` can run the configured command
* `doctor` outputs `ok/issue` lines per skill and summary `checked/issues` per root
* Language selection follows `SKILLCTL_LANG` > `LC_ALL` > `LC_MESSAGES` > `LANG`, default `ja`

---

## 13. v1 ideas (reference)

* `status --format json`
* Digest cache (performance)
* Filters (e.g. diff-only view)
