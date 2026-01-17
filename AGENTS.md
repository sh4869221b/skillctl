# AGENTS.md — 開発運用ルール（Codex/OpenCode 共通）

## 1. 目的

このリポジトリでは、仕様とテスト計画に整合した実装を最優先とする。

---

## 2. 変更の進め方（必須）

1. 仕様変更がある場合は、先に `SPEC.md` を更新する
2. 実装変更を行う
3. `TESTPLAN.md` に基づきテストを追加/更新する
4. コミット前に `cargo fmt` / `cargo clippy` / `cargo test` を通す（別セッションでも必須）

---

## 2.1 リリース手順

1. `CHANGELOG.md` の **Unreleased** を更新し、新バージョンのセクションを追加する
2. `Cargo.toml` の `version` を更新する
3. `cargo fmt` / `cargo clippy` / `cargo test` を実行する
4. リリース用コミットを作成して `main` に push する
5. `vX.Y.Z` タグを作成し push する（`release` ワークフローが起動）
6. GitHub Actions の `release` 実行結果を確認する（リリース本文は `CHANGELOG.md` の該当バージョンのみを抽出して使用）

---

## 3. 安全性（本プロジェクト固有）

* `--dry-run` は **ファイル操作ゼロ**であること（最重要）
* `push/import` は必ず **Plan（差分・操作計画）→ Execute（実行）** の段階を分離する
* 実行時の置換は「一時領域へコピー完了 → 置換」で途中状態を残さない

---

## 4. 実装の優先順位（MVP）

1. config（global_root/targets/hash/diff）
2. inventory（list）
3. digest（内容のみ + ignore + 安定化）
4. status（4状態 + table）
5. push/import（dry-run → 実行）
6. diff（外部ツール起動）

---

## 5. 出力（UX）

* `status` は table を既定とし、列は `SKILL/STATE/GLOBAL_DIGEST/TARGET_DIGEST`
* エラーは「何が起きたか」＋「次に何をすべきか」を短く示す

---

## 6. 禁止事項（MVP方針）

* 競合解決（3-way merge 等）を勝手に追加しない
* 自動バックアップや対話確認を勝手に追加しない（要件外）
* ターゲットの探索ロジック（Codex/OpenCode 固有）を本体に埋め込まない（ターゲットは任意設定）

---
