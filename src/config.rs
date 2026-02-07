use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use globset::Glob;
use serde::Deserialize;

use crate::error::{AppError, AppResult};

const CONFIG_PATH_ENV: &str = "SKILLCTL_CONFIG";
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";
const DEFAULT_CONFIG_DIR: &str = "~/.config";

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
        let path = if let Some(path) = env_var_non_empty(CONFIG_PATH_ENV) {
            expand_path(&path)?
        } else {
            default_config_path()?
        };
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> AppResult<Self> {
        let content = fs::read_to_string(path).map_err(|err| {
            let (message, hint) = match err.kind() {
                ErrorKind::NotFound => (
                    crate::tr!(
                        "設定ファイルが見つかりません: {}",
                        "Config file not found: {}",
                        path.display()
                    ),
                    Some(crate::tr!(
                        "{} を作成してから再実行してください",
                        "Create {} and retry.",
                        path.display()
                    )),
                ),
                ErrorKind::PermissionDenied => (
                    crate::tr!(
                        "設定ファイルを読み込めません: {}",
                        "Cannot read config file: {}",
                        path.display()
                    ),
                    Some(crate::tr!(
                        "ファイルの権限を確認してください",
                        "Check file permissions."
                    )),
                ),
                _ => (
                    crate::tr!(
                        "設定ファイルの読み込みに失敗しました: {}",
                        "Failed to read config file: {}",
                        path.display()
                    ),
                    Some(err.to_string()),
                ),
            };
            AppError::config(message, hint)
        })?;
        let mut config: Config = toml::from_str(&content).map_err(|err| {
            AppError::config(
                crate::tr!(
                    "設定ファイルの解析に失敗しました: {}",
                    "Failed to parse config file: {}",
                    path.display()
                ),
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
                crate::tr!(
                    "ターゲットが見つかりません: {}",
                    "Target not found: {}",
                    name
                ),
                Some(crate::tr!(
                    "targets コマンドで利用可能な名前を確認してください",
                    "Run targets to see available names."
                )),
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
        if self.diff.command.is_empty() {
            return Err(AppError::config(
                crate::tr!("diff.command が空です", "diff.command is empty"),
                Some(crate::tr!(
                    "config.toml の diff.command を設定してください",
                    "Set diff.command in config.toml"
                )),
            ));
        }
        let has_left = self.diff.command.iter().any(|arg| arg.contains("{left}"));
        let has_right = self.diff.command.iter().any(|arg| arg.contains("{right}"));
        if !has_left || !has_right {
            return Err(AppError::config(
                crate::tr!(
                    "diff.command に {{left}} と {{right}} が必要です",
                    "diff.command must include {{left}} and {{right}}"
                ),
                Some(crate::tr!(
                    "config.toml の diff.command に両方のプレースホルダを含めてください",
                    "Include both placeholders in diff.command in config.toml"
                )),
            ));
        }
        for pattern in &self.hash.ignore {
            Glob::new(pattern).map_err(|err| {
                AppError::config(
                    crate::tr!(
                        "ignore パターンが不正です: {}",
                        "Invalid ignore pattern: {}",
                        pattern
                    ),
                    Some(err.to_string()),
                )
            })?;
        }
        if self.targets.is_empty() {
            return Err(AppError::config(
                crate::tr!("targets が空です", "targets is empty"),
                Some(crate::tr!(
                    "config.toml に targets を追加してください",
                    "Add targets to config.toml"
                )),
            ));
        }
        let mut seen = HashSet::new();
        for target in &self.targets {
            if target.name.trim().is_empty() {
                return Err(AppError::config(
                    crate::tr!("targets.name が空です", "targets.name is empty"),
                    Some(crate::tr!(
                        "targets.name に一意な文字列を設定してください",
                        "Set a unique string for targets.name"
                    )),
                ));
            }
            if !seen.insert(target.name.clone()) {
                return Err(AppError::config(
                    crate::tr!(
                        "targets.name が重複しています: {}",
                        "targets.name is duplicated: {}",
                        target.name
                    ),
                    Some(crate::tr!(
                        "targets.name は一意にしてください",
                        "targets.name must be unique"
                    )),
                ));
            }
        }
        Ok(())
    }
}

fn expand_path(path: &str) -> AppResult<PathBuf> {
    let expanded = shellexpand::full(path).map_err(|err| {
        AppError::config(
            crate::tr!(
                "パス展開に失敗しました: {}",
                "Failed to expand path: {}",
                path
            ),
            Some(err.to_string()),
        )
    })?;
    Ok(PathBuf::from(expanded.as_ref()))
}

fn expand_path_pathbuf(path: &Path) -> AppResult<PathBuf> {
    let raw = path.to_string_lossy();
    expand_path(&raw)
}

fn env_var_non_empty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn default_config_path() -> AppResult<PathBuf> {
    let base = if let Some(xdg) = env_var_non_empty(XDG_CONFIG_HOME_ENV) {
        expand_path(&xdg)?
    } else if let Some(home) = env_var_non_empty("HOME") {
        expand_path(&format!("{home}/.config"))?
    } else {
        expand_path(DEFAULT_CONFIG_DIR)?
    };
    Ok(base.join("skillctl").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    use tempfile::TempDir;

    use super::*;

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            // Safety: tests serialize env mutation via env_lock.
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            // Safety: tests serialize env mutation via env_lock.
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                // Safety: tests serialize env mutation via env_lock.
                unsafe {
                    std::env::set_var(self.key, previous);
                }
            } else {
                // Safety: tests serialize env mutation via env_lock.
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn write_config(dir: &TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("config.toml");
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn config_errors_when_missing_global_root() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
[[targets]]
name = "t1"
root = "/tmp/skills"
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_missing_targets() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_targets_empty() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"
targets = []
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_target_name_duplicate() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills1"

[[targets]]
name = "t1"
root = "/tmp/skills2"
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_hash_algo_invalid() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"

[hash]
algo = "md5"
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_ignore_invalid() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"

[hash]
ignore = ["["]
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_diff_command_empty() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"

[diff]
command = []
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_errors_when_diff_command_missing_placeholder() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"

[diff]
command = ["git", "diff", "--no-index", "--", "/tmp/a", "{right}"]
"#,
        );
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn config_default_diff_command_is_set() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"
"#,
        );
        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(
            config.diff.command,
            vec![
                "git".to_string(),
                "diff".to_string(),
                "--no-index".to_string(),
                "--".to_string(),
                "{left}".to_string(),
                "{right}".to_string(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn config_errors_when_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = env_lock();
        let _lang = EnvGuard::set("SKILLCTL_LANG", "en");

        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"
"#,
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_to_string(&path).is_ok() {
            return;
        }
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(err.to_string().contains("Cannot read config file"));
    }

    #[test]
    fn config_expands_tilde_paths() {
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "~"

[[targets]]
name = "t1"
root = "~"
"#,
        );

        let config = Config::load_from_path(&path).unwrap();
        let expected = PathBuf::from(shellexpand::full("~").unwrap().into_owned());
        assert_eq!(config.global_root, expected);
        assert_eq!(config.targets[0].root, expected);
    }

    #[test]
    fn config_expands_env_paths() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let dir = TempDir::new().unwrap();
        let path = write_config(
            &dir,
            r#"
global_root = "$HOME"

[[targets]]
name = "t1"
root = "$HOME"
"#,
        );

        let config = Config::load_from_path(&path).unwrap();
        let expected = PathBuf::from(home);
        assert_eq!(config.global_root, expected);
        assert_eq!(config.targets[0].root, expected);
    }

    #[test]
    fn config_load_default_uses_xdg_config_home() {
        let _lock = env_lock();
        let dir = TempDir::new().unwrap();
        let xdg_home = dir.path().join("xdg");
        let config_dir = xdg_home.join("skillctl");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        fs::write(
            &config_path,
            r#"
global_root = "/tmp/global"

[[targets]]
name = "t1"
root = "/tmp/skills"
"#,
        )
        .unwrap();

        let _skillctl_env = EnvGuard::remove(CONFIG_PATH_ENV);
        let _xdg_env = EnvGuard::set(XDG_CONFIG_HOME_ENV, xdg_home.to_string_lossy().as_ref());

        let config = Config::load_default().unwrap();
        assert_eq!(config.global_root, PathBuf::from("/tmp/global"));
        assert_eq!(config.targets[0].root, PathBuf::from("/tmp/skills"));
    }
}
