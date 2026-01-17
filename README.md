# skillctl

[![CI](https://github.com/sh4869221b/skillctl/actions/workflows/ci.yml/badge.svg)](https://github.com/sh4869221b/skillctl/actions/workflows/ci.yml)

For Japanese, see `README.ja.md`.

`skillctl` is a CLI that copies and synchronizes agent skills from a global
(canonical) store to user-defined targets.

## Usage

### 1. Prepare a config file

The default config path is `XDG_CONFIG_HOME/skillctl/config.toml`.
If `XDG_CONFIG_HOME` is not set, `~/.config/skillctl/config.toml` is used.
If `SKILLCTL_CONFIG` is set, its path takes precedence.

```toml
global_root = "~/skills/global"

[[targets]]
name = "codex"
root = "~/.codex/skills"

[[targets]]
name = "opencode"
root = "~/.opencode/skills"

[hash]
algo = "blake3" # or "sha256"
ignore = [".git/**", "**/.DS_Store", "**/*.tmp"]

[diff]
command = ["git", "diff", "--no-index", "--", "{left}", "{right}"]
```

### 2. Install / build

```bash
# Build
cargo build

# Run (cargo run)
cargo run -- status --target codex

# Release build
cargo build --release
```

### 3. Command examples

The examples below assume `global_root = "~/skills/global"` and
`targets.name = "codex"` (`root = "~/.codex/skills"`).

```bash
# List targets
skillctl targets

# List skills (global)
skillctl list --global

# List skills (target)
skillctl list --target codex

# Status (single target)
skillctl status --target codex

# Status (all targets)
skillctl status --all

# Sync (global -> target)
skillctl push my-skill --target codex
skillctl push --all --target codex

# Import (target -> global)
skillctl import my-skill --from codex
skillctl import --all --from codex

# diff
skillctl diff my-skill --target codex
```

### Options

* `--dry-run`: list planned operations only (no file changes)
* `--prune`: include target extras for removal during `push`
* `--overwrite`: replace global during `import`

### Environment variables

* `SKILLCTL_CONFIG`: explicit config path (highest priority)
* `SKILLCTL_LANG`: message language (`ja` / `en`)
  - Falls back to `LC_ALL` / `LC_MESSAGES` / `LANG`
  - Unsupported values default to `ja`

## Behavior notes

* Digest is computed from **relative path + content** (rename or content change is diff)
* Files matching `hash.ignore` are excluded from digest
* `status` reports four states: `missing / same / diff / extra`
* `--dry-run` performs zero file operations
* Skill names must be **directory names only** (no separators, `..`, or absolute paths)

## Operations

```bash
# Check status
skillctl status --target codex

# Inspect planned ops (dry-run)
skillctl push --all --target codex --dry-run

# Execute (install/update)
skillctl push --all --target codex

# If differences remain, use diff
skillctl diff my-skill --target codex
```

## Troubleshooting

* `Config file not found` appears
  - Create `XDG_CONFIG_HOME/skillctl/config.toml` or the path set in `SKILLCTL_CONFIG`
* `Target not found` appears
  - Run `skillctl targets` to see available target names
* `Root does not exist` appears
  - Check `global_root` / `targets[].root` in `config.toml`
* `Diff target path does not exist` appears
  - Run `push` / `import` before `diff`

## Exit codes

* `0`: success
* `2`: invalid CLI arguments
* `3`: config errors (missing/invalid config, unknown target, etc.)
* `4`: execution errors (copy failure, diff launch failure, etc.)
