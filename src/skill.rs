use std::path::{Component, Path};

use crate::error::{AppError, AppResult};

pub fn validate_skill_id(skill: &str) -> AppResult<()> {
    if skill.trim().is_empty() {
        return Err(AppError::config(
            "skill が空です".to_string(),
            Some("skill 名を指定してください".to_string()),
        ));
    }
    let path = Path::new(skill);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(AppError::config(
            format!("skill が不正です: {}", skill),
            Some("スキル名はディレクトリ名のみを指定してください".to_string()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn validate_skill_id_accepts_simple_name() {
        assert!(validate_skill_id("my-skill").is_ok());
    }

    #[test]
    fn validate_skill_id_rejects_empty() {
        let err = validate_skill_id(" ").unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }

    #[test]
    fn validate_skill_id_rejects_path() {
        let err = validate_skill_id("../bad").unwrap_err();
        assert!(matches!(err, AppError::Config { .. }));
    }
}
