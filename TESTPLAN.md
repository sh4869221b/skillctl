# TESTPLAN.md — skillctl テスト計画（MVP）

## 1. テスト目標

* **内容 digest の安定性**を保証する（順序/mtime等に依存しない）
* `status` が **missing/same/diff/extra** を正しく判定する
* `--dry-run` が **不変**である（ファイル変更ゼロ）
* `push/import` が仕様通りに **収束**する（same に一致する）
* 外部 diff 起動の異常系が分かりやすい

---

## 2. テスト範囲

### 2.1 ユニットテスト

* digest（ignore、安定化、変更検知）
* 状態判定（集合演算、4状態）
* 設定ロード（必須/任意、エラー）

### 2.2 統合テスト（推奨：temp dir）

* `global_root` と `target_root` を一時ディレクトリで構築して end-to-end を検証
* CLI を直接叩かず、内部 API（status/plan/exec）を呼ぶ形でも可（実装都合で決定）

---

## 3. テストデータセット（標準形）

以下の skill を用意する（例）：

* `skill_same/`：global と target で同一内容
* `skill_diff/`：1ファイル内容が異なる
* `skill_missing/`：global にのみ存在
* `skill_extra/`：target にのみ存在

ファイル構成は最小でよい（例：`SKILL.md` 1枚＋任意ファイル）。

---

## 4. ユニットテスト詳細

### 4.1 digest

* 同一内容・同一構成 → digest 一致
* 1文字変更 → digest 不一致
* ファイル追加/削除 → digest 不一致
* ファイル列挙順序が変わっても digest が一致する
* mtime 変更のみでは digest が変わらない
* ignore パターンに一致するファイルの変更は digest に影響しない

### 4.2 設定（config.toml）

* 必須（global_root, targets）が揃っている → 正常ロード
* targets が空 → エラー
* 不正な `hash.algo` → エラー
* 不正な ignore glob → エラー
* diff.command が空配列 → エラー（diff 実行時でも可。ただし挙動は仕様化する）

### 4.3 状態判定

* 4状態（missing/same/diff/extra）を正しく判定する
* skill 名の集合（global∪target）を網羅して結果を返す
* digest 計算エラー（権限不足等）を適切に上位へ伝播する

---

## 5. 統合テスト詳細

### 5.1 status end-to-end

* 標準データセットを配置して `status` を実行し、期待通りの 4状態を得る

### 5.2 push（dry-run）

* `push --dry-run` 実行前後で target の内容が **完全に不変**（ファイル数、内容、mtime まで厳密に見るかは方針決定）
* 出力には install/update/skip が **漏れなく列挙**される

### 5.3 push（実行）

* `push` 実行後、install/update 対象は `same` へ収束する
* 既存ファイルが残骸として残らない（update で完全置換される）
* `--prune` を有効にした場合、extra が削除される（MVPに入れる場合のみ）

### 5.4 import（dry-run）

* global の内容が不変である
* 予定操作が install（＋overwrite 指定時 update）として列挙される

### 5.5 import（実行）

* 既定：global に存在しない skill のみ追加される
* `--overwrite`：同名 skill が置換され、同一（same）へ収束する

### 5.6 diff

* diff.command が有効で、両側パスが存在 → diff が起動する（終了コード 0）
* diff.command が **終了コード 2 以上** → エラー
* 両側のどちらか欠損 → 分かりやすいエラー（次の行動：push/import を促す）
* diff.command の実行ファイルが見つからない → 分かりやすいエラー

---

## 6. 異常系テスト（最低限）

* target.root が存在しない（空扱いにするかエラーにするか仕様で固定し、テストに落とす）
* global_root が存在しない（同上）
* 権限不足（read_dir/copy/exec の失敗が適切に伝播）
* 無効なターゲット名指定 → exit code 3
* skill ディレクトリが **シンボリックリンク** → エラー
* skill 名が **不正（../ やパス区切り）** → エラー

---

## 7. リリース前チェック（MVPゲート）

* `cargo fmt` / `cargo clippy` / `cargo test` が通る
* `status` の table 出力が崩れない（列・見出し）
* dry-run の不変性が統合テストで担保されている
* README に最低限の使い方（targets/status/push/import/diff）がある
