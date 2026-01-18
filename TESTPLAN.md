# TESTPLAN.md — skillctl test plan (MVP)

## 1. Test goals

* Ensure **content digest stability** (independent of order/mtime)
* `status` correctly determines **missing/same/diff/extra**
* `--dry-run` is **immutable** (zero file changes)
* `push/import` **converge** as specified
* External diff launch failures are understandable

---

## 2. Test scope

### 2.1 Unit tests

* Digest (ignore, stabilization, change detection)
* State determination (set operations, 4 states)
* Config loading (required/optional, error cases)
* Doctor (SKILL.md presence, symlinks, unsupported file types)
* Property tests (digest stability/order independence)
* Performance smoke (many files within reasonable time)

### 2.2 Integration tests (recommended: temp dir)

* Build `global_root` and `target_root` in temp dirs for end-to-end checks
* Either call internal APIs (status/plan/exec) or CLI (implementation choice)
* Error handling for permission/IO failures (e.g. unwritable target)

### 2.3 CLI E2E tests

* Snapshot `status` / `push --dry-run` output
* Validate CLI errors for invalid skill/target/missing config

---

## 3. Test dataset (standard)

Prepare these skills (example):

* `skill_same/`: identical in global and target
* `skill_diff/`: one file differs
* `skill_missing/`: only in global
* `skill_extra/`: only in target

Keep the file structure minimal (e.g. a single `SKILL.md` plus an optional file).

---

## 4. Unit test details

### 4.1 digest

* Same content + structure → digest equal
* One char change → digest different
* File add/remove → digest different
* Enumeration order does not affect digest
* Changing mtime only does not affect digest
* Changes to ignored files do not affect digest

### 4.2 config (`config.toml`)

* Required fields (global_root, targets) present → load succeeds
* With `SKILLCTL_CONFIG`, load from that path
* With `XDG_CONFIG_HOME`, read `${XDG_CONFIG_HOME}/skillctl/config.toml`
* With `SKILLCTL_LANG=en`, messages are in English
* Empty targets → error
* Invalid `hash.algo` → error
* Invalid ignore glob → error
* Empty diff.command → error (or at diff execution; define in spec)

### 4.3 state determination

* Correctly determine 4 states (missing/same/diff/extra)
* Return results for the union of skill names (global ∪ target)
* Propagate digest errors (permission, etc.) properly

### 4.4 doctor

* Missing `SKILL.md` is reported
* `SKILL.md` symlink is reported
* Symlinks inside a skill directory are reported
* Unsupported file types are reported
* OK skills report no issues

---

## 5. Integration test details

### 5.1 status end-to-end

* Run `status` with the standard dataset and get the expected 4 states

### 5.2 push (dry-run)

* `push --dry-run` leaves target **completely unchanged** (file count/content; decide mtime strictness)
* Output lists install/update/skip **without omission**

### 5.3 push (execute)

* After `push`, install/update targets converge to `same`
* No leftover files after update (full replace)
* With `--prune`, extras are removed (if included in MVP)

### 5.4 import (dry-run)

* Global remains unchanged
* Planned operations list install (and update when overwrite is set)

### 5.5 import (execute)

* Default: only add skills missing in global
* `--overwrite`: replace existing skill and converge to `same`

### 5.6 diff

* Valid diff.command and both paths exist → diff runs (exit code 0)
* diff.command exit code **1** → success (diff with changes)
* diff.command exit code **>= 2** → error
* Missing path on either side → clear error with next action guidance (push/import)
* diff.command executable missing → clear error

### 5.7 doctor

* CLI reports missing `SKILL.md` for a target/global root
* Output includes summary (`checked`, `issues`)

---

## 6. Error cases (minimum)

* target.root missing → error
* global_root missing → error
* Permission errors (read_dir/copy/exec) propagate
* Invalid target name → exit code 3
* Skill directory is a **symlink** → error
* Skill name is **invalid** (`../` or path separators) → error
* Config file missing → exit code 3

---

## 7. Pre-release checks (MVP gate)

* `cargo fmt` / `cargo clippy` / `cargo test` pass
* `status` table output is not broken (columns/headers)
* Dry-run immutability is covered by integration tests
* README has minimal usage (targets/status/doctor/push/import/diff)
