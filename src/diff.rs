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
            crate::tr!(
                "diff の対象パスが存在しません: {}",
                "Diff target path does not exist: {}",
                skill
            ),
            Some(crate::tr!(
                "push/import を実行してから再度 diff してください",
                "Run push/import before diff."
            )),
        ));
    }
    let command = &config.diff.command;
    if command.is_empty() {
        return Err(AppError::config(
            crate::tr!("diff.command が空です", "diff.command is empty"),
            Some(crate::tr!(
                "config.toml の diff.command を設定してください",
                "Set diff.command in config.toml"
            )),
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
            crate::tr!("diff.command が空です", "diff.command is empty"),
            Some(crate::tr!(
                "config.toml の diff.command を設定してください",
                "Set diff.command in config.toml"
            )),
        )
    })?;
    let status = Command::new(program).args(iter).status().map_err(|err| {
        AppError::exec(
            crate::tr!(
                "diff コマンドの起動に失敗しました",
                "Failed to start diff command"
            ),
            Some(err.to_string()),
        )
    })?;
    if let Some(code) = status.code() {
        if code > 1 {
            return Err(AppError::exec(
                crate::tr!(
                    "diff コマンドが失敗しました (exit code: {})",
                    "diff command failed (exit code: {})",
                    code
                ),
                Some(crate::tr!(
                    "diff.command を確認してください",
                    "Check diff.command"
                )),
            ));
        }
    } else if !status.success() {
        return Err(AppError::exec(
            crate::tr!(
                "diff コマンドが異常終了しました",
                "diff command terminated abnormally"
            ),
            Some(crate::tr!(
                "diff.command を確認してください",
                "Check diff.command"
            )),
        ));
    }
    Ok(())
}

fn path_to_arg(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
