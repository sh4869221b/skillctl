use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};
use crate::skill::validate_skill_id;
use crate::status::list_skills;

#[derive(Debug, Clone)]
pub struct DoctorIssue {
    pub skill: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub skills: Vec<String>,
    pub issues: Vec<DoctorIssue>,
}

pub fn doctor_root(root: &Path) -> AppResult<DoctorReport> {
    let skills = list_skills(root)?;
    let mut issues = Vec::new();
    for skill in &skills {
        if let Err(err) = validate_skill_id(skill) {
            issues.push(DoctorIssue {
                skill: skill.to_string(),
                message: err.to_string(),
            });
        }
        let skill_root = root.join(skill);
        check_skill_md(&skill_root, skill, &mut issues)?;
        check_skill_contents(&skill_root, skill, &mut issues)?;
    }
    Ok(DoctorReport {
        root: root.to_path_buf(),
        skills,
        issues,
    })
}

fn check_skill_md(skill_root: &Path, skill: &str, issues: &mut Vec<DoctorIssue>) -> AppResult<()> {
    let skill_md = skill_root.join("SKILL.md");
    match fs::symlink_metadata(&skill_md) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                issues.push(DoctorIssue {
                    skill: skill.to_string(),
                    message: crate::tr!(
                        "SKILL.md がシンボリックリンクです",
                        "SKILL.md is a symlink"
                    ),
                });
            } else if !meta.is_file() {
                issues.push(DoctorIssue {
                    skill: skill.to_string(),
                    message: crate::tr!(
                        "SKILL.md が通常ファイルではありません",
                        "SKILL.md is not a regular file"
                    ),
                });
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            issues.push(DoctorIssue {
                skill: skill.to_string(),
                message: crate::tr!("SKILL.md が見つかりません", "SKILL.md is missing"),
            });
        }
        Err(err) => {
            return Err(AppError::exec(
                crate::tr!(
                    "SKILL.md の確認に失敗しました: {}",
                    "Failed to inspect SKILL.md: {}",
                    skill_md.display()
                ),
                Some(err.to_string()),
            ));
        }
    }
    Ok(())
}

fn check_skill_contents(
    skill_root: &Path,
    skill: &str,
    issues: &mut Vec<DoctorIssue>,
) -> AppResult<()> {
    for entry in WalkDir::new(skill_root).follow_links(false) {
        let entry = entry.map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "skill の走査に失敗しました: {}",
                    "Failed to scan skill: {}",
                    skill_root.display()
                ),
                Some(err.to_string()),
            )
        })?;
        let rel = entry.path().strip_prefix(skill_root).map_err(|err| {
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
        if rel == Path::new("SKILL.md") {
            continue;
        }
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            issues.push(DoctorIssue {
                skill: skill.to_string(),
                message: crate::tr!(
                    "シンボリックリンクは未対応です: {}",
                    "Symlinks are not supported: {}",
                    rel.display()
                ),
            });
        } else if !file_type.is_dir() && !file_type.is_file() {
            issues.push(DoctorIssue {
                skill: skill.to_string(),
                message: crate::tr!(
                    "未対応のファイル種別です: {}",
                    "Unsupported file type: {}",
                    rel.display()
                ),
            });
        }
    }
    Ok(())
}

pub fn group_issues_by_skill(issues: &[DoctorIssue]) -> BTreeMap<&str, Vec<&DoctorIssue>> {
    let mut map: BTreeMap<&str, Vec<&DoctorIssue>> = BTreeMap::new();
    for issue in issues {
        map.entry(issue.skill.as_str()).or_default().push(issue);
    }
    map
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn doctor_reports_missing_skill_md() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("skill1")).unwrap();

        let report = doctor_root(root).unwrap();
        assert_eq!(report.skills, vec!["skill1".to_string()]);
        assert_eq!(report.issues.len(), 1);
        assert!(report.issues[0].message.contains("SKILL.md"));
    }

    #[test]
    fn doctor_ok_when_skill_md_present() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("skill1")).unwrap();
        fs::write(root.join("skill1/SKILL.md"), "ok").unwrap();

        let report = doctor_root(root).unwrap();
        assert_eq!(report.issues.len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn doctor_reports_symlink_inside_skill() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("skill1")).unwrap();
        fs::write(root.join("skill1/SKILL.md"), "ok").unwrap();
        let real = root.join("skill1/real.txt");
        fs::write(&real, "x").unwrap();
        let link = root.join("skill1/link.txt");
        symlink(&real, &link).unwrap();

        let report = doctor_root(root).unwrap();
        assert_eq!(report.issues.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn doctor_reports_unsupported_file_type() {
        use std::os::unix::net::UnixListener;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("skill1")).unwrap();
        fs::write(root.join("skill1/SKILL.md"), "ok").unwrap();
        let socket_path = root.join("skill1/socket.sock");
        let _listener = UnixListener::bind(&socket_path).unwrap();

        let report = doctor_root(root).unwrap();
        assert_eq!(report.issues.len(), 1);
        let message = &report.issues[0].message;
        assert!(message.contains("未対応") || message.contains("Unsupported"));
    }
}
