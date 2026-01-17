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
