use std::collections::BTreeSet;
use std::fs;
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
                format!("global に skill が存在しません: {}", skill),
                Some("list --global で一覧を確認してください".to_string()),
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
                format!("ターゲットに skill が存在しません: {}", skill),
                Some("list --target <name> で一覧を確認してください".to_string()),
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
                        format!("src が未設定です: {}", op.skill),
                        Some("実装に問題があります".to_string()),
                    )
                })?;
                let dest = op.dest.as_ref().ok_or_else(|| {
                    AppError::exec(
                        format!("dest が未設定です: {}", op.skill),
                        Some("実装に問題があります".to_string()),
                    )
                })?;
                if !dry_run {
                    replace_dir(src, dest)?;
                }
            }
            PlanKind::Prune => {
                let dest = op.dest.as_ref().ok_or_else(|| {
                    AppError::exec(
                        format!("dest が未設定です: {}", op.skill),
                        Some("実装に問題があります".to_string()),
                    )
                })?;
                if !dry_run {
                    fs::remove_dir_all(dest).map_err(|err| {
                        AppError::exec(
                            format!("削除に失敗しました: {}", dest.display()),
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

fn replace_dir(src: &Path, dest: &Path) -> AppResult<()> {
    let parent = dest.parent().ok_or_else(|| {
        AppError::exec(
            format!("親ディレクトリを特定できません: {}", dest.display()),
            Some("dest パスを確認してください".to_string()),
        )
    })?;
    let temp_dir = TempDir::new_in(parent).map_err(|err| {
        AppError::exec(
            format!("一時ディレクトリの作成に失敗しました: {}", parent.display()),
            Some(err.to_string()),
        )
    })?;
    copy_dir(src, temp_dir.path())?;
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|err| {
            AppError::exec(
                format!("既存ディレクトリの削除に失敗しました: {}", dest.display()),
                Some(err.to_string()),
            )
        })?;
    }
    fs::rename(temp_dir.path(), dest).map_err(|err| {
        AppError::exec(
            format!("ディレクトリの置換に失敗しました: {}", dest.display()),
            Some(err.to_string()),
        )
    })?;
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> AppResult<()> {
    fs::create_dir_all(dest).map_err(|err| {
        AppError::exec(
            format!("ディレクトリ作成に失敗しました: {}", dest.display()),
            Some(err.to_string()),
        )
    })?;
    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry.map_err(|err| {
            AppError::exec(
                format!("ディレクトリコピーに失敗しました: {}", src.display()),
                Some(err.to_string()),
            )
        })?;
        let rel = entry.path().strip_prefix(src).map_err(|err| {
            AppError::exec(
                format!("相対パスの取得に失敗しました: {}", entry.path().display()),
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
                    format!("ディレクトリ作成に失敗しました: {}", dest_path.display()),
                    Some(err.to_string()),
                )
            })?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    AppError::exec(
                        format!("ディレクトリ作成に失敗しました: {}", parent.display()),
                        Some(err.to_string()),
                    )
                })?;
            }
            fs::copy(entry.path(), &dest_path).map_err(|err| {
                AppError::exec(
                    format!(
                        "ファイルコピーに失敗しました: {} -> {}",
                        entry.path().display(),
                        dest_path.display()
                    ),
                    Some(err.to_string()),
                )
            })?;
        } else {
            return Err(AppError::exec(
                format!("未対応のファイル種別です: {}", entry.path().display()),
                Some("通常ファイルのみを含めてください".to_string()),
            ));
        }
    }
    Ok(())
}
