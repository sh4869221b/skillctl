use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::Config;
use crate::config::{DiffConfig, HashAlgo, HashConfig, Target};
use crate::diff::run_diff;
use crate::digest::digest_dir;
use crate::error::AppError;
use crate::status::{State, list_skills, status_for_target};
use crate::sync::{Selection, execute_plan, plan_import, plan_push};

fn make_config(global_root: PathBuf, target_root: PathBuf) -> Config {
    Config {
        global_root,
        targets: vec![Target {
            name: "t1".to_string(),
            root: target_root,
        }],
        hash: HashConfig {
            algo: HashAlgo::Blake3,
            ignore: Vec::new(),
        },
        diff: DiffConfig {
            command: vec!["diff".to_string()],
        },
    }
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn snapshot_root(root: &Path, algo: HashAlgo) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let skills = list_skills(root).unwrap();
    for skill in skills {
        let digest = digest_dir(&root.join(&skill), algo, None).unwrap();
        out.push((skill, digest));
    }
    out
}

#[test]
fn status_end_to_end() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_same/file.txt"), "same");
    write_file(&target_root.join("skill_same/file.txt"), "same");

    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];
    let rows = status_for_target(&config, target).unwrap();

    let mut lookup = rows
        .into_iter()
        .map(|row| (row.skill, row.state))
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(lookup.remove("skill_same"), Some(State::Same));
    assert_eq!(lookup.remove("skill_diff"), Some(State::Diff));
    assert_eq!(lookup.remove("skill_missing"), Some(State::Missing));
    assert_eq!(lookup.remove("skill_extra"), Some(State::Extra));
}

#[test]
fn push_dry_run_is_immutable() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];
    let before = snapshot_root(target_root, config.hash.algo);
    let plan = plan_push(&config, target, Selection::All, false).unwrap();
    execute_plan(&plan, true).unwrap();
    let after = snapshot_root(target_root, config.hash.algo);
    assert_eq!(before, after);
}

#[test]
fn push_execute_converges() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_missing/file.txt"), "m");
    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];
    let plan = plan_push(&config, target, Selection::All, false).unwrap();
    execute_plan(&plan, false).unwrap();

    let rows = status_for_target(&config, target).unwrap();
    let mut lookup = rows
        .into_iter()
        .map(|row| (row.skill, row.state))
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(lookup.remove("skill_missing"), Some(State::Same));
    assert_eq!(lookup.remove("skill_diff"), Some(State::Same));
}

#[test]
fn import_execute_add_only() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_keep/file.txt"), "global");
    write_file(&target_root.join("skill_keep/file.txt"), "target");
    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];

    let before = digest_dir(&global_root.join("skill_keep"), config.hash.algo, None).unwrap();

    let plan = plan_import(&config, target, Selection::All, false).unwrap();
    execute_plan(&plan, false).unwrap();

    let after = digest_dir(&global_root.join("skill_keep"), config.hash.algo, None).unwrap();

    assert_eq!(before, after);
    assert!(global_root.join("skill_extra").is_dir());
}

#[test]
fn push_prune_removes_extra() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_keep/file.txt"), "keep");
    write_file(&target_root.join("skill_keep/file.txt"), "keep");
    write_file(&target_root.join("skill_extra/file.txt"), "extra");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];

    let plan = plan_push(&config, target, Selection::All, true).unwrap();
    execute_plan(&plan, false).unwrap();

    assert!(!target_root.join("skill_extra").exists());
}

#[test]
fn import_overwrite_replaces() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_keep/file.txt"), "global");
    write_file(&target_root.join("skill_keep/file.txt"), "target");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];

    let plan = plan_import(&config, target, Selection::All, true).unwrap();
    execute_plan(&plan, false).unwrap();

    let global_digest =
        digest_dir(&global_root.join("skill_keep"), config.hash.algo, None).unwrap();
    let target_digest =
        digest_dir(&target_root.join("skill_keep"), config.hash.algo, None).unwrap();

    assert_eq!(global_digest, target_digest);
}

#[test]
fn import_dry_run_is_immutable() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_keep/file.txt"), "global");
    write_file(&target_root.join("skill_keep/file.txt"), "target");
    write_file(&target_root.join("skill_extra/file.txt"), "extra");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];

    let before = snapshot_root(global_root, config.hash.algo);
    let plan = plan_import(&config, target, Selection::All, false).unwrap();
    execute_plan(&plan, true).unwrap();
    let after = snapshot_root(global_root, config.hash.algo);

    assert_eq!(before, after);
}

#[test]
fn diff_errors_when_missing() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_missing/file.txt"), "g");

    let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    let target = &config.targets[0];

    let err = run_diff(&config, target, "skill_missing").unwrap_err();
    assert!(matches!(err, AppError::Exec { .. }));
}

#[test]
fn diff_errors_when_command_missing() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let mut config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    config.diff.command = vec!["__no_such_command__".to_string()];
    let target = &config.targets[0];

    let err = run_diff(&config, target, "skill_diff").unwrap_err();
    assert!(matches!(err, AppError::Exec { .. }));
}

#[cfg(unix)]
#[test]
fn diff_errors_when_exit_code_gt1() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let mut config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    config.diff.command = vec!["sh".to_string(), "-c".to_string(), "exit 2".to_string()];
    let target = &config.targets[0];

    let err = run_diff(&config, target, "skill_diff").unwrap_err();
    assert!(matches!(err, AppError::Exec { .. }));
}

#[test]
fn diff_runs_when_command_ok() {
    let global_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();
    let target_root = target_dir.path();

    write_file(&global_root.join("skill_diff/file.txt"), "g");
    write_file(&target_root.join("skill_diff/file.txt"), "t");

    let mut config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
    config.diff.command = vec!["true".to_string()];
    let target = &config.targets[0];

    run_diff(&config, target, "skill_diff").unwrap();
}

#[test]
fn status_errors_when_global_missing() {
    let target_dir = TempDir::new().unwrap();
    let target_root = target_dir.path();

    write_file(&target_root.join("skill_extra/file.txt"), "e");

    let config = make_config(PathBuf::from("/no/such/global"), target_root.to_path_buf());
    let target = &config.targets[0];

    let err = status_for_target(&config, target).unwrap_err();
    assert!(matches!(err, AppError::Config { .. }));
}

#[test]
fn status_errors_when_target_missing() {
    let global_dir = TempDir::new().unwrap();
    let global_root = global_dir.path();

    write_file(&global_root.join("skill_missing/file.txt"), "m");

    let config = make_config(global_root.to_path_buf(), PathBuf::from("/no/such/target"));
    let target = &config.targets[0];

    let err = status_for_target(&config, target).unwrap_err();
    assert!(matches!(err, AppError::Config { .. }));
}
