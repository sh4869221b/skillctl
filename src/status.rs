use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;

use tabwriter::TabWriter;

use crate::config::{Config, Target};
use crate::digest::{build_ignore_set, digest_dir, short_digest};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Missing,
    Same,
    Diff,
    Extra,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            State::Missing => "missing",
            State::Same => "same",
            State::Diff => "diff",
            State::Extra => "extra",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone)]
pub struct StatusRow {
    pub skill: String,
    pub state: State,
    pub global_digest: Option<String>,
    pub target_digest: Option<String>,
}

pub fn list_skills(root: &Path) -> AppResult<Vec<String>> {
    ensure_root_dir(root)?;
    let mut skills = Vec::new();
    for entry in fs::read_dir(root).map_err(|err| {
        AppError::config(
            format!("ディレクトリを読み込めません: {}", root.display()),
            Some(err.to_string()),
        )
    })? {
        let entry = entry.map_err(|err| {
            AppError::config(
                format!("ディレクトリを読み込めません: {}", root.display()),
                Some(err.to_string()),
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            AppError::exec(
                format!("ディレクトリを読み込めません: {}", root.display()),
                Some(err.to_string()),
            )
        })?;
        if file_type.is_symlink() {
            return Err(AppError::exec(
                format!("シンボリックリンクは未対応です: {}", path.display()),
                Some("通常のディレクトリを配置してください".to_string()),
            ));
        }
        if file_type.is_dir()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            skills.push(name.to_string());
        }
    }
    skills.sort();
    Ok(skills)
}

pub fn status_for_target(config: &Config, target: &Target) -> AppResult<Vec<StatusRow>> {
    ensure_root_dir(&config.global_root)?;
    ensure_root_dir(&target.root)?;

    let global_skills = list_skills(&config.global_root)?;
    let target_skills = list_skills(&target.root)?;

    let mut all = BTreeSet::new();
    all.extend(global_skills.iter().cloned());
    all.extend(target_skills.iter().cloned());

    let ignore = build_ignore_set(&config.hash.ignore)?;
    let mut rows = Vec::new();
    for skill in all {
        let global_path = config.global_root.join(&skill);
        let target_path = target.root.join(&skill);
        let global_exists = global_path.is_dir();
        let target_exists = target_path.is_dir();
        let (state, global_digest, target_digest) = match (global_exists, target_exists) {
            (true, true) => {
                let g = digest_dir(&global_path, config.hash.algo, ignore.as_ref())?;
                let t = digest_dir(&target_path, config.hash.algo, ignore.as_ref())?;
                if g == t {
                    (State::Same, Some(g), Some(t))
                } else {
                    (State::Diff, Some(g), Some(t))
                }
            }
            (true, false) => (
                State::Missing,
                Some(digest_dir(&global_path, config.hash.algo, ignore.as_ref())?),
                None,
            ),
            (false, true) => (
                State::Extra,
                None,
                Some(digest_dir(&target_path, config.hash.algo, ignore.as_ref())?),
            ),
            (false, false) => continue,
        };
        rows.push(StatusRow {
            skill,
            state,
            global_digest,
            target_digest,
        });
    }
    Ok(rows)
}

pub fn render_status_table(rows: &[StatusRow]) -> AppResult<String> {
    let mut tw = TabWriter::new(vec![]);
    writeln!(tw, "SKILL\tSTATE\tGLOBAL_DIGEST\tTARGET_DIGEST").map_err(|err| {
        AppError::exec(
            "status 出力の整形に失敗しました".to_string(),
            Some(err.to_string()),
        )
    })?;
    for row in rows {
        let g = row
            .global_digest
            .as_deref()
            .map(short_digest)
            .unwrap_or_else(|| "-".to_string());
        let t = row
            .target_digest
            .as_deref()
            .map(short_digest)
            .unwrap_or_else(|| "-".to_string());
        writeln!(tw, "{}\t{}\t{}\t{}", row.skill, row.state, g, t).map_err(|err| {
            AppError::exec(
                "status 出力の整形に失敗しました".to_string(),
                Some(err.to_string()),
            )
        })?;
    }
    let output = tw.into_inner().map_err(|err| {
        AppError::exec(
            "status 出力の整形に失敗しました".to_string(),
            Some(err.to_string()),
        )
    })?;
    String::from_utf8(output).map_err(|err| {
        AppError::exec(
            "status 出力の整形に失敗しました".to_string(),
            Some(err.to_string()),
        )
    })
}

fn ensure_root_dir(root: &Path) -> AppResult<()> {
    if !root.is_dir() {
        return Err(AppError::config(
            format!("root が存在しません: {}", root.display()),
            Some("config.toml のパスを確認してください".to_string()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;
    use crate::config::{Config, DiffConfig, HashAlgo, HashConfig, Target};

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

    #[test]
    fn status_detects_states() {
        let global_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();
        let global_root = global_dir.path();
        let target_root = target_dir.path();

        fs::create_dir_all(global_root.join("skill_same")).unwrap();
        fs::create_dir_all(target_root.join("skill_same")).unwrap();
        fs::write(global_root.join("skill_same/file.txt"), "same").unwrap();
        fs::write(target_root.join("skill_same/file.txt"), "same").unwrap();

        fs::create_dir_all(global_root.join("skill_diff")).unwrap();
        fs::create_dir_all(target_root.join("skill_diff")).unwrap();
        fs::write(global_root.join("skill_diff/file.txt"), "g").unwrap();
        fs::write(target_root.join("skill_diff/file.txt"), "t").unwrap();

        fs::create_dir_all(global_root.join("skill_missing")).unwrap();
        fs::write(global_root.join("skill_missing/file.txt"), "m").unwrap();

        fs::create_dir_all(target_root.join("skill_extra")).unwrap();
        fs::write(target_root.join("skill_extra/file.txt"), "e").unwrap();

        let config = make_config(global_root.to_path_buf(), target_root.to_path_buf());
        let target = &config.targets[0];
        let rows = status_for_target(&config, target).unwrap();
        let find_state = |name: &str| {
            rows.iter()
                .find(|row| row.skill == name)
                .map(|row| row.state)
                .unwrap()
        };

        assert_eq!(find_state("skill_same"), State::Same);
        assert_eq!(find_state("skill_diff"), State::Diff);
        assert_eq!(find_state("skill_missing"), State::Missing);
        assert_eq!(find_state("skill_extra"), State::Extra);
    }

    #[cfg(unix)]
    #[test]
    fn list_skills_errors_on_symlink() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        let real = dir.path().join("real");
        fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        symlink(&real, &link).unwrap();

        let err = list_skills(dir.path()).unwrap_err();
        assert!(matches!(err, AppError::Exec { .. }));
    }
}
