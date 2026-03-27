# skillctl Review Findings Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the five review findings so `skillctl` matches `SPEC.md`/`TESTPLAN.md`, preserves data during sync, and has regression coverage for the repaired paths.

**Architecture:** Keep the existing CLI surface and module boundaries, but tighten invariants at the module edges: sync replacement must become rollback-safe, doctor/diff must validate skill roots consistently, and digest/i18n must align with the documented rules. Add focused regression tests close to the touched modules first, then extend CLI E2E coverage only where user-visible output or multi-target behavior changed.

**Tech Stack:** Rust 2024, `cargo test`, `cargo clippy`, existing unit/core E2E/CLI snapshot tests, `tempfile`, `walkdir`

---

## File Map

- Modify: `src/sync.rs`
  - Replace the current destructive swap with a rollback-safe directory replacement flow.
- Modify: `src/core_e2e_tests.rs`
  - Add sync regression coverage and diff behavior coverage.
- Modify: `src/status.rs`
  - Extract or add a reusable root-entry inspection path so `doctor` can report invalid skill roots without aborting.
- Modify: `src/doctor.rs`
  - Report root-level symlink skill directories as `issue` lines instead of exiting early.
- Modify: `src/diff.rs`
  - Reject symlink-backed skill roots explicitly before invoking the diff command.
- Modify: `src/digest.rs`
  - Hash relative paths without lossy UTF-8 collapse on Unix and add regression tests.
- Modify: `src/i18n.rs`
  - Make unsupported locale values, including `C` / `POSIX`, fall back to Japanese per spec.
- Modify: `tests/cli_e2e.rs`
  - Add multi-target CLI coverage and stronger dry-run immutability assertions.
- Modify: `tests/snapshots/*.snap`
  - Update or add snapshots for `status --all` / `doctor --all` if output changes.
- Optional modify: `README.md`
  - Only if implementation clarifies wording that is currently ambiguous; no behavior doc change is expected for the reviewed findings.

### Task 1: Make Directory Replacement Rollback-Safe

**Files:**
- Modify: `src/sync.rs`
- Modify: `src/core_e2e_tests.rs`

- [ ] **Step 1: Write a failing regression test for replacement failure safety**

Add a test-oriented seam in `src/sync.rs` design first, then write a test in `src/core_e2e_tests.rs` that proves the old destination still exists if the final swap fails.

Suggested test shape:

```rust
#[test]
fn replace_dir_keeps_existing_dest_when_final_swap_fails() {
    // Arrange src + existing dest contents.
    // Trigger a deterministic failure in the final publish step.
    // Assert destination still contains the original payload.
}
```

- [ ] **Step 2: Run the targeted test to confirm it fails on current behavior**

Run: `cargo test replace_dir_keeps_existing_dest_when_final_swap_fails -- --exact`

Expected: FAIL because current `replace_dir()` deletes `dest` before `rename()`.

- [ ] **Step 3: Implement a non-destructive swap sequence**

Update `src/sync.rs` so publish uses this order:

1. Copy `src` into a temp sibling under `dest.parent()`
2. If `dest` exists, rename `dest` to a backup sibling in the same parent
3. Rename temp directory into `dest`
4. Remove the backup only after publish succeeds
5. If step 3 fails, restore backup back to `dest` before returning the error
6. If the restore in step 5 also fails, return an error that includes both failures and explicitly states that manual recovery is required; keep the backup path in the message or hint so the operator can restore it manually

Keep `dry_run` behavior unchanged: `execute_plan(..., true)` must still perform zero filesystem mutation.

- [ ] **Step 4: Add/adjust success-path coverage**

Strengthen existing tests in `src/core_e2e_tests.rs` so they still verify:

- `push_execute_converges`
- `import_overwrite_replaces`
- `push_prune_removes_extra`

and that update leaves no leftover files after replacement.

- [ ] **Step 5: Re-run the focused sync test set**

Run:

```bash
cargo test replace_dir_keeps_existing_dest_when_final_swap_fails -- --exact
cargo test push_execute_converges -- --exact
cargo test import_overwrite_replaces -- --exact
cargo test push_prune_removes_extra -- --exact
```

Expected: PASS.

### Task 2: Make `doctor` and `diff` Validate Skill Roots Consistently

**Files:**
- Modify: `src/status.rs`
- Modify: `src/doctor.rs`
- Modify: `src/diff.rs`
- Modify: `src/core_e2e_tests.rs`
- Modify: `tests/cli_e2e.rs`

- [ ] **Step 1: Write the failing tests first**

Add coverage for these cases:

- `doctor` reports a root-level symlink skill as an `issue` line and still prints summary
- `diff` rejects a symlink-backed skill root with exit code 4 / `AppError::Exec`

Suggested test names:

```rust
#[test]
fn doctor_reports_root_symlink_skill_instead_of_aborting() {}

#[test]
fn diff_rejects_symlink_skill_root() {}
```

- [ ] **Step 2: Run just those tests to confirm the current failures**

Run:

```bash
cargo test doctor_reports_root_symlink_skill_instead_of_aborting -- --exact
cargo test diff_rejects_symlink_skill_root -- --exact
```

Expected: FAIL. Current `doctor` aborts through `list_skills()`, and `diff` allows symlink directories through `is_dir()`.

- [ ] **Step 3: Refactor root entry inspection without breaking existing callers**

Introduce one of these minimal patterns:

- a new helper in `src/status.rs` that enumerates root entries with file type metadata, or
- a stricter helper for `status/list` plus a report-friendly helper for `doctor`

Design goal:

- `status`, `list`, `push`, and `import` may still hard-fail on unsupported root entries
- `doctor` must be able to surface them as issues and continue

- [ ] **Step 4: Update `doctor` to report instead of abort**

Make `doctor_root()` include invalid root-level skill entries in `issues`, then continue checking the valid directory skills so the CLI can still emit:

- `issue <skill> ...`
- `ok <skill>`
- `checked: <count> issues: <count>`

- [ ] **Step 5: Update `diff` to reject non-normal directories**

Before diff execution, inspect both skill roots with `symlink_metadata()` and reject:

- missing paths
- symlinks
- non-directory file types

Use the existing error style with a short next-step hint.

- [ ] **Step 6: Re-run the focused coverage**

Run:

```bash
cargo test doctor_reports_root_symlink_skill_instead_of_aborting -- --exact
cargo test diff_rejects_symlink_skill_root -- --exact
cargo test diff_errors_when_missing -- --exact
cargo test diff_runs_when_command_ok -- --exact
cargo test diff_errors_when_exit_code_gt1 -- --exact
```

Expected: PASS.

### Task 3: Align Digest and Locale Handling with Spec

**Files:**
- Modify: `src/digest.rs`
- Modify: `src/i18n.rs`

- [ ] **Step 1: Add failing tests for both spec mismatches**

Write:

- a Unix-only digest regression test proving two distinct non-UTF8 relative paths do not collapse to the same hash input
- an i18n test proving `SKILLCTL_LANG`, `LC_ALL`, `LC_MESSAGES`, or `LANG` values of `C` / `POSIX` fall back to `Lang::Ja`

Suggested shapes:

```rust
#[cfg(unix)]
#[test]
fn digest_distinguishes_non_utf8_relative_paths() {}

#[test]
fn current_lang_defaults_to_ja_for_c_locale() {}
```

- [ ] **Step 2: Run the focused tests to verify current failure**

Run:

```bash
cargo test digest_distinguishes_non_utf8_relative_paths -- --exact
cargo test current_lang_defaults_to_ja_for_c_locale -- --exact
```

Expected: FAIL on current implementation.

- [ ] **Step 3: Remove lossy path hashing on Unix**

In `src/digest.rs`, stop using `to_string_lossy()` for hash input on Unix. Preferred approach:

- hash each relative path component using raw `OsStrExt::as_bytes()`
- insert `/` separators explicitly between components
- keep sorting stable with an ordering that does not collapse distinct byte sequences

On non-Unix platforms, keep the current path normalization unless a better native-byte approach is available without broad refactoring.

- [ ] **Step 4: Make locale fallback match the spec**

In `src/i18n.rs`, treat only explicit `ja` and `en` families as supported values. Everything else, including `c` and `posix`, should resolve to `Lang::Ja`.

- [ ] **Step 5: Re-run the focused tests**

Run:

```bash
cargo test digest_distinguishes_non_utf8_relative_paths -- --exact
cargo test current_lang_defaults_to_ja_for_c_locale -- --exact
cargo test digest::tests
cargo test i18n::tests
```

Expected: PASS.

### Task 4: Close the CLI Regression Gaps

**Files:**
- Modify: `tests/cli_e2e.rs`
- Modify: `tests/snapshots/cli_e2e__*.snap`

- [ ] **Step 1: Add missing multi-target CLI coverage**

Extend the fixture helpers so CLI tests can create two configured targets. Then add snapshot or string-based tests for:

- `status --all`
- `doctor --all`

Each should verify labeled sections appear for every target in a deterministic order.

- [ ] **Step 2: Strengthen dry-run immutability coverage**

Change the existing dry-run test to snapshot the entire fixture root (or another scope that includes config + both roots), not just the target directory, before and after:

```rust
let before = snapshot_dir(root.path());
// run push --dry-run
let after = snapshot_dir(root.path());
assert_eq!(before, after);
```

This keeps the current no-mutation guarantee but broadens the blast radius being checked.

- [ ] **Step 3: Regenerate snapshots**

Run:

```bash
INSTA_UPDATE=always cargo test --test cli_e2e
```

Expected: snapshot files update only for the new `--all` coverage or intended output changes.

- [ ] **Step 4: Verify the snapshot diff manually**

Review changed `tests/snapshots/*.snap` files and confirm:

- labels are stable
- summary lines are preserved
- no unintended localization or formatting changes slipped in

### Task 5: Final Verification and Review Loop

**Files:**
- No new functional files expected

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all`

Expected: no diff after formatting rerun.

- [ ] **Step 2: Run lint**

Run: `cargo clippy --all-targets --all-features`

Expected: PASS or only pre-existing accepted warnings. Remove the current test-only `useless_vec` warning while touching `src/digest.rs`.

- [ ] **Step 3: Run full tests**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 4: Review user-visible docs only if behavior text changed**

Check:

- `README.md`
- `README.ja.md`

If wording already matches implementation after the fixes, leave docs untouched.

- [ ] **Step 5: Request code review before merge**

Dispatch reviewer coverage with these scopes:

- sync safety and rollback behavior
- doctor/diff path validation consistency
- digest/i18n spec alignment and new regression tests

Expected: no outstanding high/medium findings before merge.
