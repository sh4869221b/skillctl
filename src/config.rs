use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, AppResult};

const DEFAULT_CONFIG_PATH: &str = "~/.config/skillctl/config.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub global_root: PathBuf,
    pub targets: Vec<Target>,
    #[serde(default)]
    pub hash: HashConfig,
    #[serde(default)]
    pub diff: DiffConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub name: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HashConfig {
    #[serde(default)]
    pub algo: HashAlgo,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgo {
    #[default]
    Blake3,
    Sha256,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiffConfig {
    #[serde(default = "default_diff_command")]
    pub command: Vec<String>,
}

fn default_diff_command() -> Vec<String> {
    vec![
        "git".to_string(),
        "diff".to_string(),
        "--no-index".to_string(),
        "--".to_string(),
        "{left}".to_string(),
        "{right}".to_string(),
    ]
}

impl Default for HashConfig {
    fn default() -> Self {
        Self {
            algo: HashAlgo::Blake3,
            ignore: Vec::new(),
        }
    }
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            command: default_diff_command(),
        }
    }
}

impl Config {
    pub fn load_default() -> AppResult<Self> {
        let path = expand_path(DEFAULT_CONFIG_PATH)?;
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> AppResult<Self> {
        let content = fs::read_to_string(path).map_err(|_| {
            AppError::config(
                format!("設定ファイルが見つかりません: {}", path.display()),
                Some(format!(
                    "{} を作成してから再実行してください",
                    DEFAULT_CONFIG_PATH
                )),
            )
        })?;
        let mut config: Config = toml::from_str(&content).map_err(|err| {
            AppError::config(
                format!("設定ファイルの解析に失敗しました: {}", path.display()),
                Some(err.to_string()),
            )
        })?;
        config.expand_paths()?;
        config.validate()?;
        Ok(config)
    }

    pub fn target_by_name(&self, name: &str) -> AppResult<&Target> {
        self.targets.iter().find(|t| t.name == name).ok_or_else(|| {
            AppError::config(
                format!("ターゲットが見つかりません: {}", name),
                Some("targets コマンドで利用可能な名前を確認してください".to_string()),
            )
        })
    }

    fn expand_paths(&mut self) -> AppResult<()> {
        self.global_root = expand_path_pathbuf(&self.global_root)?;
        for target in &mut self.targets {
            target.root = expand_path_pathbuf(&target.root)?;
        }
        Ok(())
    }

    fn validate(&self) -> AppResult<()> {
        if self.targets.is_empty() {
            return Err(AppError::config(
                "targets が空です".to_string(),
                Some("config.toml に targets を追加してください".to_string()),
            ));
        }
        let mut seen = HashSet::new();
        for target in &self.targets {
            if target.name.trim().is_empty() {
                return Err(AppError::config(
                    "targets.name が空です".to_string(),
                    Some("targets.name に一意な文字列を設定してください".to_string()),
                ));
            }
            if !seen.insert(target.name.clone()) {
                return Err(AppError::config(
                    format!("targets.name が重複しています: {}", target.name),
                    Some("targets.name は一意にしてください".to_string()),
                ));
            }
        }
        Ok(())
    }
}

fn expand_path(path: &str) -> AppResult<PathBuf> {
    let expanded = shellexpand::full(path).map_err(|err| {
        AppError::config(
            format!("パス展開に失敗しました: {}", path),
            Some(err.to_string()),
        )
    })?;
    Ok(PathBuf::from(expanded.as_ref()))
}

fn expand_path_pathbuf(path: &Path) -> AppResult<PathBuf> {
    let raw = path.to_string_lossy();
    expand_path(&raw)
}
