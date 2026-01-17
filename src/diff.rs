use std::path::Path;
use std::process::Command;

use crate::config::{Config, Target};
use crate::error::{AppError, AppResult};
use crate::skill::validate_skill_id;

pub fn run_diff(config: &Config, target: &Target, skill: &str) -> AppResult<()> {
    validate_skill_id(skill)?;
    let left = config.global_root.join(skill);
    let right = target.root.join(skill);
    if !left.is_dir() || !right.is_dir() {
        return Err(AppError::exec(
            format!("diff の対象パスが存在しません: {}", skill),
            Some("push/import を実行してから再度 diff してください".to_string()),
        ));
    }
    let command = &config.diff.command;
    if command.is_empty() {
        return Err(AppError::config(
            "diff.command が空です".to_string(),
            Some("config.toml の diff.command を設定してください".to_string()),
        ));
    }
    let mut args = Vec::new();
    for arg in command {
        let replaced = arg
            .replace("{left}", &path_to_arg(&left))
            .replace("{right}", &path_to_arg(&right));
        args.push(replaced);
    }
    let mut iter = args.into_iter();
    let program = iter.next().ok_or_else(|| {
        AppError::config(
            "diff.command が空です".to_string(),
            Some("config.toml の diff.command を設定してください".to_string()),
        )
    })?;
    let status = Command::new(program).args(iter).status().map_err(|err| {
        AppError::exec(
            "diff コマンドの起動に失敗しました".to_string(),
            Some(err.to_string()),
        )
    })?;
    if let Some(code) = status.code() {
        if code > 1 {
            return Err(AppError::exec(
                format!("diff コマンドが失敗しました (exit code: {})", code),
                Some("diff.command を確認してください".to_string()),
            ));
        }
    } else if !status.success() {
        return Err(AppError::exec(
            "diff コマンドが異常終了しました".to_string(),
            Some("diff.command を確認してください".to_string()),
        ));
    }
    Ok(())
}

fn path_to_arg(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
