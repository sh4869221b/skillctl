use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::config::{Config, Target};
use crate::digest::{build_ignore_set, digest_dir};
use crate::error::{AppError, AppResult};
use crate::skill::validate_skill_id;
use crate::status::list_skills;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    Install,
    Update,
    Skip,
    Prune,
}

#[derive(Debug, Clone)]
pub struct PlanOp {
    pub kind: PlanKind,
    pub skill: String,
    pub src: Option<PathBuf>,
    pub dest: Option<PathBuf>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub ops: Vec<PlanOp>,
}

#[derive(Debug, Clone, Copy)]
pub enum Selection<'a> {
    All,
    One(&'a str),
}

pub fn plan_push(
    config: &Config,
    target: &Target,
    selection: Selection<'_>,
    prune: bool,
) -> AppResult<Plan> {
    let global_skills = list_skills(&config.global_root)?;
    let target_skills = list_skills(&target.root)?;

    let mut skills = BTreeSet::new();
    match selection {
        Selection::All => {
            skills.extend(global_skills.iter().cloned());
            if prune {
                skills.extend(target_skills.iter().cloned());
            }
        }
        Selection::One(skill) => {
            validate_skill_id(skill)?;
            skills.insert(skill.to_string());
        }
    }

    if let Selection::One(skill) = selection {
        validate_skill_id(skill)?;
        let in_global = global_skills.iter().any(|s| s == skill);
        let in_target = target_skills.iter().any(|s| s == skill);
        if !in_global && (!prune || !in_target) {
            return Err(AppError::exec(
                crate::tr!(
                    "global に skill が存在しません: {}",
                    "Skill does not exist in global: {}",
                    skill
                ),
                Some(crate::tr!(
                    "list --global で一覧を確認してください",
                    "Run list --global to see available skills."
                )),
            ));
        }
    }

    let ignore = build_ignore_set(&config.hash.ignore)?;
    let mut ops = Vec::new();
    for skill in skills {
        let global_path = config.global_root.join(&skill);
        let target_path = target.root.join(&skill);
        let global_exists = global_path.is_dir();
        let target_exists = target_path.is_dir();
        let op = match (global_exists, target_exists) {
            (true, false) => PlanOp {
                kind: PlanKind::Install,
                skill,
                src: Some(global_path),
                dest: Some(target_path),
                note: None,
            },
            (true, true) => {
                let g = digest_dir(&global_path, config.hash.algo, ignore.as_ref())?;
                let t = digest_dir(&target_path, config.hash.algo, ignore.as_ref())?;
                if g == t {
                    PlanOp {
                        kind: PlanKind::Skip,
                        skill,
                        src: None,
                        dest: None,
                        note: None,
                    }
                } else {
                    PlanOp {
                        kind: PlanKind::Update,
                        skill,
                        src: Some(global_path),
                        dest: Some(target_path),
                        note: None,
                    }
                }
            }
            (false, true) => {
                if prune {
                    PlanOp {
                        kind: PlanKind::Prune,
                        skill,
                        src: None,
                        dest: Some(target_path),
                        note: None,
                    }
                } else {
                    PlanOp {
                        kind: PlanKind::Skip,
                        skill,
                        src: None,
                        dest: None,
                        note: Some("extra".to_string()),
                    }
                }
            }
            (false, false) => continue,
        };
        ops.push(op);
    }
    Ok(Plan { ops })
}

pub fn plan_import(
    config: &Config,
    target: &Target,
    selection: Selection<'_>,
    overwrite: bool,
) -> AppResult<Plan> {
    let target_skills = list_skills(&target.root)?;

    let mut skills = BTreeSet::new();
    match selection {
        Selection::All => {
            skills.extend(target_skills.iter().cloned());
        }
        Selection::One(skill) => {
            validate_skill_id(skill)?;
            skills.insert(skill.to_string());
        }
    }

    if let Selection::One(skill) = selection {
        validate_skill_id(skill)?;
        let in_target = target_skills.iter().any(|s| s == skill);
        if !in_target {
            return Err(AppError::exec(
                crate::tr!(
                    "ターゲットに skill が存在しません: {}",
                    "Skill does not exist in target: {}",
                    skill
                ),
                Some(crate::tr!(
                    "list --target <name> で一覧を確認してください",
                    "Run list --target <name> to see available skills."
                )),
            ));
        }
    }

    let ignore = build_ignore_set(&config.hash.ignore)?;
    let mut ops = Vec::new();
    for skill in skills {
        let global_path = config.global_root.join(&skill);
        let target_path = target.root.join(&skill);
        let global_exists = global_path.is_dir();
        let target_exists = target_path.is_dir();
        let op = match (global_exists, target_exists) {
            (false, true) => PlanOp {
                kind: PlanKind::Install,
                skill,
                src: Some(target_path),
                dest: Some(global_path),
                note: None,
            },
            (true, true) => {
                let g = digest_dir(&global_path, config.hash.algo, ignore.as_ref())?;
                let t = digest_dir(&target_path, config.hash.algo, ignore.as_ref())?;
                if g == t {
                    PlanOp {
                        kind: PlanKind::Skip,
                        skill,
                        src: None,
                        dest: None,
                        note: None,
                    }
                } else if overwrite {
                    PlanOp {
                        kind: PlanKind::Update,
                        skill,
                        src: Some(target_path),
                        dest: Some(global_path),
                        note: None,
                    }
                } else {
                    PlanOp {
                        kind: PlanKind::Skip,
                        skill,
                        src: None,
                        dest: None,
                        note: Some("diff".to_string()),
                    }
                }
            }
            (true, false) => PlanOp {
                kind: PlanKind::Skip,
                skill,
                src: None,
                dest: None,
                note: Some("missing".to_string()),
            },
            (false, false) => continue,
        };
        ops.push(op);
    }
    Ok(Plan { ops })
}

pub fn execute_plan(plan: &Plan, dry_run: bool) -> AppResult<()> {
    for op in &plan.ops {
        match op.kind {
            PlanKind::Install | PlanKind::Update => {
                let src = op.src.as_ref().ok_or_else(|| {
                    AppError::exec(
                        crate::tr!("src が未設定です: {}", "src is not set: {}", op.skill),
                        Some(crate::tr!(
                            "実装に問題があります",
                            "There is an implementation bug."
                        )),
                    )
                })?;
                let dest = op.dest.as_ref().ok_or_else(|| {
                    AppError::exec(
                        crate::tr!("dest が未設定です: {}", "dest is not set: {}", op.skill),
                        Some(crate::tr!(
                            "実装に問題があります",
                            "There is an implementation bug."
                        )),
                    )
                })?;
                if !dry_run {
                    replace_dir(src, dest)?;
                }
            }
            PlanKind::Prune => {
                let dest = op.dest.as_ref().ok_or_else(|| {
                    AppError::exec(
                        crate::tr!("dest が未設定です: {}", "dest is not set: {}", op.skill),
                        Some(crate::tr!(
                            "実装に問題があります",
                            "There is an implementation bug."
                        )),
                    )
                })?;
                if !dry_run {
                    fs::remove_dir_all(dest).map_err(|err| {
                        AppError::exec(
                            crate::tr!(
                                "削除に失敗しました: {}",
                                "Failed to remove: {}",
                                dest.display()
                            ),
                            Some(err.to_string()),
                        )
                    })?;
                }
            }
            PlanKind::Skip => {}
        }
    }
    Ok(())
}

pub fn summarize_plan(plan: &Plan) -> Vec<String> {
    let mut lines = Vec::new();
    for op in &plan.ops {
        let label = match op.kind {
            PlanKind::Install => "install",
            PlanKind::Update => "update",
            PlanKind::Skip => "skip",
            PlanKind::Prune => "prune",
        };
        let mut line = format!("{} {}", label, op.skill);
        if let Some(note) = &op.note {
            line.push_str(&format!(" ({})", note));
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
#[derive(Debug, Default)]
struct RenameTestHooks {
    fail_publish_once: bool,
    fail_restore_once: bool,
}

#[cfg(test)]
fn rename_test_hooks() -> &'static std::sync::Mutex<RenameTestHooks> {
    static HOOKS: std::sync::OnceLock<std::sync::Mutex<RenameTestHooks>> =
        std::sync::OnceLock::new();
    HOOKS.get_or_init(|| std::sync::Mutex::new(RenameTestHooks::default()))
}

#[cfg(test)]
pub(crate) fn fail_next_publish_rename_for_test() {
    let mut hooks = rename_test_hooks().lock().unwrap();
    hooks.fail_publish_once = true;
}

#[cfg(test)]
pub(crate) fn fail_next_restore_rename_for_test() {
    let mut hooks = rename_test_hooks().lock().unwrap();
    hooks.fail_restore_once = true;
}

#[cfg(test)]
fn maybe_fail_rename_for_test(phase: RenamePhase) -> io::Result<()> {
    let mut hooks = rename_test_hooks().lock().unwrap();
    let should_fail = match phase {
        RenamePhase::Publish => &mut hooks.fail_publish_once,
        RenamePhase::Restore => &mut hooks.fail_restore_once,
        RenamePhase::Backup => return Ok(()),
    };
    if *should_fail {
        *should_fail = false;
        Err(io::Error::other(format!(
            "forced {:?} rename failure",
            phase
        )))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum RenamePhase {
    Backup,
    Publish,
    Restore,
}

fn rename_dir(from: &Path, to: &Path, _phase: RenamePhase) -> io::Result<()> {
    #[cfg(test)]
    maybe_fail_rename_for_test(_phase)?;
    fs::rename(from, to)
}

fn replace_dir(src: &Path, dest: &Path) -> AppResult<()> {
    let parent = dest.parent().ok_or_else(|| {
        AppError::exec(
            crate::tr!(
                "親ディレクトリを特定できません: {}",
                "Failed to determine parent directory: {}",
                dest.display()
            ),
            Some(crate::tr!(
                "dest パスを確認してください",
                "Check the dest path."
            )),
        )
    })?;
    let temp_dir = TempDir::new_in(parent).map_err(|err| {
        AppError::exec(
            crate::tr!(
                "一時ディレクトリの作成に失敗しました: {}",
                "Failed to create temp directory: {}",
                parent.display()
            ),
            Some(err.to_string()),
        )
    })?;
    copy_dir(src, temp_dir.path())?;
    let backup_path = if dest.exists() {
        let backup = next_backup_path(dest)?;
        rename_dir(dest, &backup, RenamePhase::Backup).map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "既存ディレクトリの退避に失敗しました: {}",
                    "Failed to back up existing directory: {}",
                    dest.display()
                ),
                Some(err.to_string()),
            )
        })?;
        Some(backup)
    } else {
        None
    };

    match rename_dir(temp_dir.path(), dest, RenamePhase::Publish) {
        Ok(()) => {
            if let Some(backup) = backup_path {
                fs::remove_dir_all(&backup).map_err(|err| {
                    AppError::exec(
                        crate::tr!(
                            "バックアップの削除に失敗しました: {}",
                            "Failed to remove backup directory: {}",
                            backup.display()
                        ),
                        Some(err.to_string()),
                    )
                })?;
            }
            Ok(())
        }
        Err(publish_err) => {
            if let Some(backup) = backup_path {
                match rename_dir(&backup, dest, RenamePhase::Restore) {
                    Ok(()) => Err(AppError::exec(
                        crate::tr!(
                            "ディレクトリの置換に失敗しました: {}",
                            "Failed to replace directory: {}",
                            dest.display()
                        ),
                        Some(publish_err.to_string()),
                    )),
                    Err(restore_err) => Err(AppError::exec(
                        crate::tr!(
                            "ディレクトリの置換と復旧に失敗しました: {}",
                            "Failed to replace and restore directory: {}",
                            dest.display()
                        ),
                        Some(format!(
                            "{}; {}. {}",
                            publish_err,
                            restore_err,
                            crate::tr!(
                                "手動で {} を {} に戻してください",
                                "Manually rename {} back to {}.",
                                backup.display(),
                                dest.display()
                            )
                        )),
                    )),
                }
            } else {
                Err(AppError::exec(
                    crate::tr!(
                        "ディレクトリの置換に失敗しました: {}",
                        "Failed to replace directory: {}",
                        dest.display()
                    ),
                    Some(publish_err.to_string()),
                ))
            }
        }
    }
}

fn next_backup_path(dest: &Path) -> AppResult<PathBuf> {
    let parent = dest.parent().ok_or_else(|| {
        AppError::exec(
            crate::tr!(
                "親ディレクトリを特定できません: {}",
                "Failed to determine parent directory: {}",
                dest.display()
            ),
            Some(crate::tr!(
                "dest パスを確認してください",
                "Check the dest path."
            )),
        )
    })?;
    let base = dest
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            AppError::exec(
                crate::tr!(
                    "ディレクトリ名を特定できません: {}",
                    "Failed to determine directory name: {}",
                    dest.display()
                ),
                Some(crate::tr!(
                    "dest パスを確認してください",
                    "Check the dest path."
                )),
            )
        })?;
    for attempt in 0..1024 {
        let candidate = parent.join(format!(".skillctl-backup-{base}-{attempt}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(AppError::exec(
        crate::tr!(
            "バックアップパスを作成できません: {}",
            "Failed to allocate backup path: {}",
            dest.display()
        ),
        Some(crate::tr!(
            "親ディレクトリ内の .skillctl-backup-* を確認してください",
            "Check existing .skillctl-backup-* entries in the parent directory."
        )),
    ))
}

fn copy_dir(src: &Path, dest: &Path) -> AppResult<()> {
    fs::create_dir_all(dest).map_err(|err| {
        AppError::exec(
            crate::tr!(
                "ディレクトリ作成に失敗しました: {}",
                "Failed to create directory: {}",
                dest.display()
            ),
            Some(err.to_string()),
        )
    })?;
    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry.map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "ディレクトリコピーに失敗しました: {}",
                    "Failed to copy directory: {}",
                    src.display()
                ),
                Some(err.to_string()),
            )
        })?;
        let rel = entry.path().strip_prefix(src).map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "相対パスの取得に失敗しました: {}",
                    "Failed to get relative path: {}",
                    entry.path().display()
                ),
                Some(err.to_string()),
            )
        })?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest_path = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path).map_err(|err| {
                AppError::exec(
                    crate::tr!(
                        "ディレクトリ作成に失敗しました: {}",
                        "Failed to create directory: {}",
                        dest_path.display()
                    ),
                    Some(err.to_string()),
                )
            })?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    AppError::exec(
                        crate::tr!(
                            "ディレクトリ作成に失敗しました: {}",
                            "Failed to create directory: {}",
                            parent.display()
                        ),
                        Some(err.to_string()),
                    )
                })?;
            }
            fs::copy(entry.path(), &dest_path).map_err(|err| {
                AppError::exec(
                    crate::tr!(
                        "ファイルコピーに失敗しました: {} -> {}",
                        "Failed to copy file: {} -> {}",
                        entry.path().display(),
                        dest_path.display()
                    ),
                    Some(err.to_string()),
                )
            })?;
        } else {
            return Err(AppError::exec(
                crate::tr!(
                    "未対応のファイル種別です: {}",
                    "Unsupported file type: {}",
                    entry.path().display()
                ),
                Some(crate::tr!(
                    "通常ファイルのみを含めてください",
                    "Include only regular files."
                )),
            ));
        }
    }
    Ok(())
}
