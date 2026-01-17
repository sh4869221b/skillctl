# skillctl

[![CI](https://github.com/sh4869221b/skillctl/actions/workflows/ci.yml/badge.svg)](https://github.com/sh4869221b/skillctl/actions/workflows/ci.yml)

`skillctl` は、グローバル（正本）に集約した agent skills を、ユーザーが定義した
ターゲットへコピー同期する CLI です。

## 使い方

### 1. 設定ファイルを用意する

既定の設定パスは `~/.config/skillctl/config.toml` です。

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

### 2. インストール / ビルド

```bash
# ビルド
cargo build

# 実行（cargo run）
cargo run -- status --target codex

# リリースビルド
cargo build --release
```

### 3. コマンド例

以下の例は、上記の `global_root = "~/skills/global"` と
`targets.name = "codex"`（`root = "~/.codex/skills"`）を前提としています。

```bash
# ターゲット一覧
skillctl targets

# スキル一覧（global）
skillctl list --global

# スキル一覧（target）
skillctl list --target codex

# 状態確認（単一ターゲット）
skillctl status --target codex

# 状態確認（全ターゲット）
skillctl status --all

# 同期（global -> target）
skillctl push my-skill --target codex
skillctl push --all --target codex

# 取り込み（target -> global）
skillctl import my-skill --from codex
skillctl import --all --from codex

# diff
skillctl diff my-skill --target codex
```

### オプション

* `--dry-run`：操作予定の列挙のみ（ファイル操作は行わない）
* `--prune`：`push` 時に target の extra を削除対象に含める
* `--overwrite`：`import` 時に global を置換する

## 振る舞いのポイント

* digest は **相対パス＋内容**で計算し、ファイル名や内容の変更は差分扱い
* `hash.ignore` に一致するファイルは digest 計算から除外
* `status` は `missing / same / diff / extra` の 4 状態を出力
* `--dry-run` はファイル操作ゼロ
* skill 名は **ディレクトリ名のみ**（パス区切りや `..`、絶対パスは不可）

## 運用例

```bash
# まず状態を確認
skillctl status --target codex

# 予定操作を確認（dry-run）
skillctl push --all --target codex --dry-run

# 実行（install/update）
skillctl push --all --target codex

# 差分が残っていれば diff で確認
skillctl diff my-skill --target codex
```

## トラブルシュート

* `設定ファイルが見つかりません` が出る  
  - `~/.config/skillctl/config.toml` を作成して再実行してください
* `ターゲットが見つかりません` が出る  
  - `skillctl targets` で利用可能なターゲット名を確認してください
* `root が存在しません` が出る  
  - `config.toml` の `global_root` / `targets[].root` を確認してください
* `diff の対象パスが存在しません` が出る  
  - `push` / `import` で同期後に `diff` を実行してください

## 終了コード

* `0`：正常
* `2`：CLI 引数不正
* `3`：設定不正（config 不在/解析不能/ターゲット未定義など）
* `4`：実行エラー（コピー失敗、diff 起動失敗など）
