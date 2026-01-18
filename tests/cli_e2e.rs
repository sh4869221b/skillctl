use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn escape_toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_config(path: &Path, global_root: &Path, target_root: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let body = format!(
        r#"global_root = "{}"

[[targets]]
name = "t1"
root = "{}"
"#,
        escape_toml_path(global_root),
        escape_toml_path(target_root)
    );
    fs::write(path, body).unwrap();
}

fn write_config_with_diff_command(
    path: &Path,
    global_root: &Path,
    target_root: &Path,
    diff_command: &[&str],
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let command_list = diff_command
        .iter()
        .map(|value| format!("\"{}\"", escape_toml_string(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let body = format!(
        r#"global_root = "{}"

[[targets]]
name = "t1"
root = "{}"

[diff]
command = [{}]
"#,
        escape_toml_path(global_root),
        escape_toml_path(target_root),
        command_list
    );
    fs::write(path, body).unwrap();
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn normalize_output(output: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(output).to_string();
    stdout.replace("\r\n", "\n")
}

fn set_config_env(cmd: &mut assert_cmd::Command, path: &Path) {
    cmd.env("SKILLCTL_CONFIG", path);
    cmd.env("SKILLCTL_LANG", "ja");
}

fn setup_fixture() -> (TempDir, PathBuf, PathBuf, PathBuf) {
    let root = TempDir::new().unwrap();
    let global_root = root.path().join("global");
    let target_root = root.path().join("target");
    let config_path = root.path().join("config.toml");
    fs::create_dir_all(&global_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    write_config(&config_path, &global_root, &target_root);
    (root, global_root, target_root, config_path)
}

fn setup_fixture_with_diff_command(diff_command: &[&str]) -> (TempDir, PathBuf, PathBuf, PathBuf) {
    let root = TempDir::new().unwrap();
    let global_root = root.path().join("global");
    let target_root = root.path().join("target");
    let config_path = root.path().join("config.toml");
    fs::create_dir_all(&global_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    write_config_with_diff_command(&config_path, &global_root, &target_root, diff_command);
    (root, global_root, target_root, config_path)
}

#[cfg(windows)]
fn diff_exit_one_command() -> Vec<&'static str> {
    vec!["cmd", "/C", "exit 1"]
}

#[cfg(not(windows))]
fn diff_exit_one_command() -> Vec<&'static str> {
    vec!["sh", "-c", "exit 1"]
}

#[test]
fn status_outputs_table_snapshot() {
    let (_root, global_root, target_root, config_path) = setup_fixture();

    write_file(&global_root.join("skill_same/file.txt"), "same");
    write_file(&target_root.join("skill_same/file.txt"), "same");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");
    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("status").arg("--target").arg("t1");
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = normalize_output(&output);
    insta::assert_snapshot!(stdout);
}

#[test]
fn push_dry_run_snapshot() {
    let (_root, global_root, target_root, config_path) = setup_fixture();

    write_file(&global_root.join("skill_same/file.txt"), "same");
    write_file(&target_root.join("skill_same/file.txt"), "same");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");
    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("push")
        .arg("--all")
        .arg("--target")
        .arg("t1")
        .arg("--dry-run")
        .arg("--prune");
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = normalize_output(&output);
    insta::assert_snapshot!(stdout);
}

#[test]
fn push_execute_converges() {
    let (_root, global_root, target_root, config_path) = setup_fixture();

    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("push").arg("--all").arg("--target").arg("t1");
    cmd.assert().success();

    let missing = fs::read_to_string(target_root.join("skill_missing/file.txt")).unwrap();
    let diff = fs::read_to_string(target_root.join("skill_diff/file.txt")).unwrap();
    assert_eq!(missing, "m");
    assert_eq!(diff, "g");
}

#[test]
fn import_execute_add_only() {
    let (_root, global_root, target_root, config_path) = setup_fixture();

    write_file(&global_root.join("skill_keep/file.txt"), "global");
    write_file(&target_root.join("skill_keep/file.txt"), "target");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("import").arg("--all").arg("--from").arg("t1");
    cmd.assert().success();

    let keep = fs::read_to_string(global_root.join("skill_keep/file.txt")).unwrap();
    let extra = fs::read_to_string(global_root.join("skill_extra/file.txt")).unwrap();
    assert_eq!(keep, "global");
    assert_eq!(extra, "e");
}

#[test]
fn doctor_global_snapshot() {
    let (_root, global_root, _target_root, config_path) = setup_fixture();

    write_file(&global_root.join("a_missing/notes.txt"), "x");
    write_file(&global_root.join("b_ok/SKILL.md"), "ok");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("doctor").arg("--global");
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = normalize_output(&output);
    insta::assert_snapshot!(stdout);
}

#[test]
fn diff_exit_code_one_is_success() {
    let diff_command = diff_exit_one_command();
    let (_root, global_root, target_root, config_path) =
        setup_fixture_with_diff_command(&diff_command);

    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("diff").arg("skill_diff").arg("--target").arg("t1");
    cmd.assert().success();
}

#[test]
fn diff_rejects_invalid_skill_cli() {
    let (_root, _global_root, _target_root, config_path) = setup_fixture();

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("diff").arg("../bad").arg("--target").arg("t1");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("skill が不正です"));
}

#[test]
fn status_rejects_unknown_target() {
    let (_root, _global_root, _target_root, config_path) = setup_fixture();

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("status").arg("--target").arg("nope");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("ターゲットが見つかりません"));
}

#[test]
fn status_errors_when_config_missing() {
    let root = TempDir::new().unwrap();
    let config_path = root.path().join("missing.toml");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_config_env(&mut cmd, &config_path);
    cmd.arg("status").arg("--target").arg("t1");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("設定ファイルが見つかりません"));
}
