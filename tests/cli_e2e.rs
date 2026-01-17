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
    cmd.env("HOME", root.path().join("home"));
    cmd.arg("status").arg("--target").arg("t1");
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output).to_string();
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
    cmd.env("HOME", root.path().join("home"));
    cmd.arg("push")
        .arg("--all")
        .arg("--target")
        .arg("t1")
        .arg("--dry-run")
        .arg("--prune");
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output).to_string();
    insta::assert_snapshot!(stdout);
}

#[test]
fn diff_rejects_invalid_skill_cli() {
    let (root, _global_root, _target_root) = setup_fixture();

    let mut cmd = cargo_bin_cmd!("skillctl");
    cmd.env("HOME", root.path().join("home"));
    cmd.arg("diff").arg("../bad").arg("--target").arg("t1");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("skill が不正です"));
}
