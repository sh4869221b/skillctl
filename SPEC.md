# SPEC.md — skillctl 仕様（MVP → v1）

## 1. 概要

`skillctl` は、**グローバル（正本）**に集約した agent skills を、ユーザーが自由に定義した **ターゲット（name + root directory）**へ **コピー同期**する CLI です。

* **管理単位**：`<root>/<skill_id>/...` の **ディレクトリ**
* **同一性判定**：**相対パス＋内容**に基づくディレクトリ digest（ハッシュ）
* **差分確認**：外部 diff ツールを設定して起動
* **安全機構**：`--dry-run` のみ（操作予定の完全列挙、ファイル操作はゼロ）

---

## 2. ゴール / 非ゴール

### 2.1 ゴール

* グローバル正本とターゲットの **差分状態**を可視化できる
* グローバル正本からターゲットへ **収束**（install/update/skip）できる
* 必要に応じてターゲットからグローバルへ **取り込み**できる（既定は追加のみ）
* 差分がある skill を **diff ツールで比較**できる

### 2.2 非ゴール（MVPでは対象外）

* 3-way merge 等の競合解決
* 自動バックアップ、対話確認（interactive）
* リモート取得（GitHub 等から直接インストール）
* 監視・常駐・自動同期

---

## 3. 用語

* **global_root**：正本 skills 置き場
* **target**：同期先（任意に設定できる名前＋ルートパス）
* **skill_id**：ディレクトリ名（例：`git-release`）。パス区切りや `..`、絶対パスは不可
* **digest**：ディレクトリの相対パス＋内容から算出したハッシュ
* **state**：global と target の比較結果（missing/same/diff/extra）

---

## 4. ディレクトリモデル

### 4.1 グローバル（正本）

* `global_root/<skill_id>/...`
* `skill_id` は **通常のディレクトリ**であること（シンボリックリンクは不可）

### 4.2 ターゲット（同期先）

* `targets[].root/<skill_id>/...`

※ Codex/OpenCode 固有の探索パスは **このツールの前提ではない**（あくまでターゲットの例として登録できる）。

---

## 5. 設定（config.toml）

### 5.1 既定パス

* `~/.config/skillctl/config.toml`
* `SKILLCTL_CONFIG` 環境変数がある場合はそのパスを優先する

### 5.2 スキーマ（必須）

* `global_root: string`
* `targets: array`

  * `name: string`（一意）
  * `root: string`

### 5.3 スキーマ（任意）

* `[hash]`

  * `algo: "blake3" | "sha256"`（既定：`blake3`）
  * `ignore: string[]`（glob パターン。既定：空）
* `[diff]`

  * `command: string[]`（argv 形式。既定：`git diff --no-index -- {left} {right}`）

### 5.4 パス展開

* `~` と環境変数（`$VAR` / `${VAR}`）を展開して解決する

---

## 6. Digest（相対パス＋内容ハッシュ）仕様

### 6.1 対象

* skill ディレクトリ配下の **通常ファイル**
* **相対パス**（安定化ルールに従い正規化）
* **内容**（バイト列）
* **シンボリックリンクは対象外**（エラー）

### 6.2 不含（ハッシュに含めない）

* mtime、所有者、パーミッションなどメタデータ
* ディレクトリの列挙順（順序は正規化する）

### 6.3 安定化ルール

1. `ignore` 適用後の対象ファイルを列挙
2. **相対パス昇順**にソート
3. ハッシュ入力に「相対パス」と「内容」を投入（ファイル名の変更も差分扱い）

### 6.4 ignore

* `hash.ignore` の glob に一致する相対パスは除外する
* 既定の推奨（例）：`.git/**`, `**/.DS_Store`, `**/*.tmp`

---

## 7. 状態判定（status）

### 7.1 状態

* `missing`：global にあり target にない
* `same`：双方にあり digest 一致
* `diff`：双方にあり digest 不一致
* `extra`：target にのみ存在（global にない）

### 7.2 出力（table 既定）

* 列：`SKILL | STATE | GLOBAL_DIGEST | TARGET_DIGEST`
* digest は短縮表示可（例：先頭3 + 末尾3）

---

## 8. 同期（push / import）仕様

* push/import は **Plan（差分・操作計画）→ Execute（実行）** の二段階を分離する

### 8.1 push（global → target）

* 入力：`<skill_id>` または `--all`、`--target <name>`
* 判定：

  * `missing` → **install**
  * `diff` → **update**
  * `same` → **skip**
* 更新方式（実装要件）：

  * 一時領域へコピー完了後に置換する（途中状態を残さない）
* `--dry-run`：

  * install/update/skip（＋必要なら prune）を **完全列挙**する
  * ファイル操作は行わない

#### `--prune`（任意）

* target にのみ存在する skill（`extra`）を削除対象に含める
* 既定は prune しない（安全寄り）

### 8.2 import（target → global）

* 入力：`<skill_id>` または `--all`、`--from <name>`
* 既定挙動：

  * global に存在しない skill のみ **取り込み（install）**
* `--overwrite`：

  * 同名が存在しても global を置換する（明示時のみ）
* `--dry-run`：

  * 予定操作を列挙し、ファイル操作は行わない

---

## 9. diff 仕様

* `diff.command`（argv 配列）に `{left}` `{right}` プレースホルダを用意
* `skillctl diff <skill> --target <name>` 実行時に置換して起動
* どちらかのパスが存在しない場合はエラー（次の行動を示すメッセージを出す）
* diff の終了コードは **0/1 を成功扱い**、それ以外はエラー

---

## 10. CLI コマンド仕様（MVP）

### 10.1 コマンド一覧

* `targets`
* `list --global | --target <name>`
* `status --target <name> | --all`
* `push [<skill>|--all] --target <name> [--dry-run] [--prune]`
* `import [<skill>|--all] --from <name> [--dry-run] [--overwrite]`
* `diff <skill> --target <name>`

### 10.2 終了コード

* `0`：正常
* `2`：CLI 引数不正
* `3`：設定不正（config 不在/解析不能/ターゲット未定義など）
* `4`：実行エラー（コピー失敗、diff 起動失敗など）

---

## 11. 受け入れ基準（MVP）

* `status` が 4状態を正しく出せる
* `push --dry-run` が予定操作を漏れなく列挙し、ファイルが一切変化しない
* `push` 実行後、対象 skill は `same` に収束する
* `import` は既定で追加のみ、`--overwrite` で置換できる
* `diff` が設定コマンドを起動できる

---

## 12. v1 の拡張候補（参考）

* `status --format json`
* `doctor`（SKILL.md 有無、命名規約チェック等）
* digest キャッシュ（性能改善）
* フィルタ（diff のみ表示など）
