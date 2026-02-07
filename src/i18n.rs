use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

pub fn current_lang() -> Lang {
    if let Ok(value) = env::var("SKILLCTL_LANG") {
        return parse_lang(&value);
    }
    if let Ok(value) = env::var("LC_ALL") {
        return parse_lang(&value);
    }
    if let Ok(value) = env::var("LC_MESSAGES") {
        return parse_lang(&value);
    }
    if let Ok(value) = env::var("LANG") {
        return parse_lang(&value);
    }
    Lang::Ja
}

fn parse_lang(value: &str) -> Lang {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Lang::Ja;
    }
    let lowered = trimmed.to_ascii_lowercase();
    let primary = lowered.split(['.', '@'].as_ref()).next().unwrap_or("");
    let lang = primary.split(['_', '-'].as_ref()).next().unwrap_or("");
    match lang {
        "ja" => Lang::Ja,
        "en" | "c" | "posix" => Lang::En,
        _ => Lang::Ja,
    }
}

#[macro_export]
macro_rules! tr {
    ($ja:expr, $en:expr $(, $args:expr)* $(,)?) => {{
        match $crate::i18n::current_lang() {
            $crate::i18n::Lang::Ja => format!($ja $(, $args)*),
            $crate::i18n::Lang::En => format!($en $(, $args)*),
        }
    }};
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

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

    #[test]
    fn current_lang_prefers_skillctl_lang() {
        let _lock = env_lock();
        let _skillctl = EnvGuard::set("SKILLCTL_LANG", "en");
        let _lc_all = EnvGuard::set("LC_ALL", "ja_JP.UTF-8");
        let _lc_messages = EnvGuard::set("LC_MESSAGES", "ja_JP.UTF-8");
        let _lang = EnvGuard::set("LANG", "ja_JP.UTF-8");

        assert_eq!(current_lang(), Lang::En);
    }

    #[test]
    fn current_lang_falls_back_to_lc_all() {
        let _lock = env_lock();
        let _skillctl = EnvGuard::remove("SKILLCTL_LANG");
        let _lc_all = EnvGuard::set("LC_ALL", "en_US.UTF-8");
        let _lc_messages = EnvGuard::set("LC_MESSAGES", "ja_JP.UTF-8");
        let _lang = EnvGuard::set("LANG", "ja_JP.UTF-8");

        assert_eq!(current_lang(), Lang::En);
    }

    #[test]
    fn current_lang_falls_back_to_lc_messages() {
        let _lock = env_lock();
        let _skillctl = EnvGuard::remove("SKILLCTL_LANG");
        let _lc_all = EnvGuard::remove("LC_ALL");
        let _lc_messages = EnvGuard::set("LC_MESSAGES", "en_US.UTF-8");
        let _lang = EnvGuard::set("LANG", "ja_JP.UTF-8");

        assert_eq!(current_lang(), Lang::En);
    }

    #[test]
    fn current_lang_falls_back_to_lang() {
        let _lock = env_lock();
        let _skillctl = EnvGuard::remove("SKILLCTL_LANG");
        let _lc_all = EnvGuard::remove("LC_ALL");
        let _lc_messages = EnvGuard::remove("LC_MESSAGES");
        let _lang = EnvGuard::set("LANG", "en_US.UTF-8");

        assert_eq!(current_lang(), Lang::En);
    }

    #[test]
    fn current_lang_defaults_to_ja_on_unsupported() {
        let _lock = env_lock();
        let _skillctl = EnvGuard::set("SKILLCTL_LANG", "fr_FR");

        assert_eq!(current_lang(), Lang::Ja);
    }
}
