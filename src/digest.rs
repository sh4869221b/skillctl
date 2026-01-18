use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use sha2::Digest as Sha2Digest;
use walkdir::WalkDir;

use crate::config::HashAlgo;
use crate::error::{AppError, AppResult};

pub fn build_ignore_set(patterns: &[String]) -> AppResult<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| {
            AppError::config(
                crate::tr!(
                    "ignore パターンが不正です: {}",
                    "Invalid ignore pattern: {}",
                    pattern
                ),
                Some(err.to_string()),
            )
        })?;
        builder.add(glob);
    }
    let set = builder.build().map_err(|err| {
        AppError::config(
            crate::tr!(
                "ignore パターンの構築に失敗しました",
                "Failed to build ignore patterns"
            ),
            Some(err.to_string()),
        )
    })?;
    Ok(Some(set))
}

pub fn digest_dir(path: &Path, algo: HashAlgo, ignore: Option<&GlobSet>) -> AppResult<String> {
    if !path.is_dir() {
        return Err(AppError::exec(
            crate::tr!(
                "ディレクトリが見つかりません: {}",
                "Directory not found: {}",
                path.display()
            ),
            Some(crate::tr!(
                "対象パスを確認してください",
                "Check the target path."
            )),
        ));
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "ファイル走査に失敗しました: {}",
                    "Failed to scan files: {}",
                    path.display()
                ),
                Some(err.to_string()),
            )
        })?;
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
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
        let rel = entry.path().strip_prefix(path).map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "相対パスの取得に失敗しました: {}",
                    "Failed to get relative path: {}",
                    entry.path().display()
                ),
                Some(err.to_string()),
            )
        })?;
        let rel_string = normalize_rel_path(rel);
        if let Some(set) = ignore
            && set.is_match(&rel_string)
        {
            continue;
        }
        files.push((rel_string, entry.path().to_path_buf()));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = DigestHasher::new(algo);
    for (rel, full) in files {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hash_file(&mut hasher, &full)?;
        hasher.update(b"\0");
    }
    Ok(hasher.finalize_hex())
}

pub fn short_digest(digest: &str) -> String {
    if digest.len() <= 6 {
        digest.to_string()
    } else {
        format!("{}...{}", &digest[..3], &digest[digest.len() - 3..])
    }
}

fn normalize_rel_path(path: &Path) -> String {
    let mut out = String::new();
    for (i, part) in path.components().enumerate() {
        if i > 0 {
            out.push('/');
        }
        out.push_str(part.as_os_str().to_string_lossy().as_ref());
    }
    out
}

fn hash_file(hasher: &mut DigestHasher, path: &Path) -> AppResult<()> {
    let file = File::open(path).map_err(|err| {
        AppError::exec(
            crate::tr!(
                "ファイルの読み込みに失敗しました: {}",
                "Failed to read file: {}",
                path.display()
            ),
            Some(err.to_string()),
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut buf = [0u8; 8192];
    loop {
        let read = reader.read(&mut buf).map_err(|err| {
            AppError::exec(
                crate::tr!(
                    "ファイルの読み込みに失敗しました: {}",
                    "Failed to read file: {}",
                    path.display()
                ),
                Some(err.to_string()),
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(())
}

#[allow(clippy::large_enum_variant)]
enum DigestHasher {
    Blake3(blake3::Hasher),
    Sha256(sha2::Sha256),
}

impl DigestHasher {
    fn new(algo: HashAlgo) -> Self {
        match algo {
            HashAlgo::Blake3 => Self::Blake3(blake3::Hasher::new()),
            HashAlgo::Sha256 => Self::Sha256(sha2::Sha256::new()),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Blake3(hasher) => {
                hasher.update(data);
            }
            Self::Sha256(hasher) => {
                hasher.update(data);
            }
        }
    }

    fn finalize_hex(self) -> String {
        match self {
            Self::Blake3(hasher) => hasher.finalize().to_hex().to_string(),
            Self::Sha256(hasher) => {
                let bytes = hasher.finalize();
                to_hex(&bytes)
            }
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use proptest::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::config::HashAlgo;

    #[test]
    fn digest_stable_on_mtime() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "hello").unwrap();
        let first = digest_dir(dir.path(), HashAlgo::Blake3, None).unwrap();
        fs::write(&path, "hello").unwrap();
        let second = digest_dir(dir.path(), HashAlgo::Blake3, None).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn digest_changes_on_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, "hello").unwrap();
        let first = digest_dir(dir.path(), HashAlgo::Sha256, None).unwrap();
        fs::write(&path, "hello2").unwrap();
        let second = digest_dir(dir.path(), HashAlgo::Sha256, None).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn digest_ignores_patterns() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("skip.tmp"), "noise").unwrap();
        let ignore = build_ignore_set(&vec!["**/*.tmp".to_string()])
            .unwrap()
            .unwrap();
        let first = digest_dir(dir.path(), HashAlgo::Blake3, Some(&ignore)).unwrap();
        fs::write(dir.path().join("skip.tmp"), "changed").unwrap();
        let second = digest_dir(dir.path(), HashAlgo::Blake3, Some(&ignore)).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn digest_distinguishes_nested_paths() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        fs::create_dir_all(dir_a.path().join("a")).unwrap();
        fs::write(dir_a.path().join("a/b.txt"), "x").unwrap();
        fs::write(dir_b.path().join("ab.txt"), "x").unwrap();
        let first = digest_dir(dir_a.path(), HashAlgo::Blake3, None).unwrap();
        let second = digest_dir(dir_b.path(), HashAlgo::Blake3, None).unwrap();
        assert_ne!(first, second);
    }

    proptest! {
        #[test]
        fn digest_stable_for_same_content(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("a.txt");
            fs::write(&path, &bytes).unwrap();
            let first = digest_dir(dir.path(), HashAlgo::Blake3, None).unwrap();
            fs::write(&path, &bytes).unwrap();
            let second = digest_dir(dir.path(), HashAlgo::Blake3, None).unwrap();
            prop_assert_eq!(first, second);
        }

        #[test]
        fn digest_changes_on_different_content(
            a in proptest::collection::vec(any::<u8>(), 0..256),
            b in proptest::collection::vec(any::<u8>(), 0..256),
        ) {
            prop_assume!(a != b);
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("a.txt");
            fs::write(&path, &a).unwrap();
            let first = digest_dir(dir.path(), HashAlgo::Sha256, None).unwrap();
            fs::write(&path, &b).unwrap();
            let second = digest_dir(dir.path(), HashAlgo::Sha256, None).unwrap();
            prop_assert_ne!(first, second);
        }

        #[test]
        fn digest_order_independent(contents in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..64), 1..8)) {
            let dir_a = TempDir::new().unwrap();
            let dir_b = TempDir::new().unwrap();

            for (i, bytes) in contents.iter().enumerate() {
                fs::write(dir_a.path().join(format!("file_{i}.txt")), bytes).unwrap();
            }
            for (i, bytes) in contents.iter().enumerate().rev() {
                fs::write(dir_b.path().join(format!("file_{i}.txt")), bytes).unwrap();
            }

            let first = digest_dir(dir_a.path(), HashAlgo::Blake3, None).unwrap();
            let second = digest_dir(dir_b.path(), HashAlgo::Blake3, None).unwrap();
            prop_assert_eq!(first, second);
        }
    }

    #[test]
    fn digest_performance_smoke() {
        let dir = TempDir::new().unwrap();
        for i in 0..1000 {
            fs::write(dir.path().join(format!("file_{i}.txt")), "x").unwrap();
        }
        let start = Instant::now();
        let _ = digest_dir(dir.path(), HashAlgo::Blake3, None).unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
