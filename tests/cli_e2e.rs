use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

fn escape_toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

fn write_config(home: &Path, global_root: &Path, target_root: &Path) {
    let config_dir = home.join(".config").join("skillctl");
    fs::create_dir_all(&config_dir).unwrap();
    let body = format!(
        r#"global_root = "{}"

[[targets]]
name = "t1"
root = "{}"
"#,
        escape_toml_path(global_root),
        escape_toml_path(target_root)
    );
    fs::write(config_dir.join("config.toml"), body).unwrap();
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

fn set_home_env(cmd: &mut assert_cmd::Command, home: &Path) {
    let home_str = home.to_string_lossy().to_string();
    cmd.env("HOME", &home_str);
    cmd.env("USERPROFILE", &home_str);
    if cfg!(windows) {
        if let Some((drive, rest)) = split_windows_drive(&home_str) {
            cmd.env("HOMEDRIVE", drive);
            cmd.env("HOMEPATH", rest);
        }
    }
}

fn split_windows_drive(path: &str) -> Option<(String, String)> {
    let bytes = path.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' {
        return None;
    }
    let drive = path[..2].to_string();
    let mut rest = path[2..].to_string();
    if rest.is_empty() {
        rest = "\\".to_string();
    } else if !rest.starts_with('\\') && !rest.starts_with('/') {
        rest = format!("\\{rest}");
    }
    Some((drive, rest))
}

fn setup_fixture() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    let global_root = root.path().join("global");
    let target_root = root.path().join("target");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&global_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    write_config(&home, &global_root, &target_root);
    (root, global_root, target_root)
}

#[test]
fn status_outputs_table_snapshot() {
    let (root, global_root, target_root) = setup_fixture();

    write_file(&global_root.join("skill_same/file.txt"), "same");
    write_file(&target_root.join("skill_same/file.txt"), "same");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");
    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_home_env(&mut cmd, &root.path().join("home"));
    cmd.arg("status").arg("--target").arg("t1");
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = normalize_output(&output);
    insta::assert_snapshot!(stdout);
}

#[test]
fn push_dry_run_snapshot() {
    let (root, global_root, target_root) = setup_fixture();

    write_file(&global_root.join("skill_same/file.txt"), "same");
    write_file(&target_root.join("skill_same/file.txt"), "same");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");
    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_home_env(&mut cmd, &root.path().join("home"));
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
fn diff_rejects_invalid_skill_cli() {
    let (root, _global_root, _target_root) = setup_fixture();

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_home_env(&mut cmd, &root.path().join("home"));
    cmd.arg("diff").arg("../bad").arg("--target").arg("t1");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("skill が不正です"));
}

#[test]
fn status_rejects_unknown_target() {
    let (root, _global_root, _target_root) = setup_fixture();

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_home_env(&mut cmd, &root.path().join("home"));
    cmd.arg("status").arg("--target").arg("nope");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("ターゲットが見つかりません"));
}

#[test]
fn status_errors_when_config_missing() {
    let root = TempDir::new().unwrap();

    let mut cmd = cargo_bin_cmd!("skillctl");
    set_home_env(&mut cmd, root.path());
    cmd.arg("status").arg("--target").arg("t1");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("設定ファイルが見つかりません"));
}
