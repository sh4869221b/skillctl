use std::path::{Component, Path};

use crate::error::{AppError, AppResult};

pub fn validate_skill_id(skill: &str) -> AppResult<()> {
    if skill.trim().is_empty() {
        return Err(AppError::config(
            crate::tr!("skill が空です", "skill is empty"),
            Some(crate::tr!(
                "skill 名を指定してください",
                "Provide a skill name."
            )),
        ));
    }
    let path = Path::new(skill);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(AppError::config(
            crate::tr!("skill が不正です: {}", "Skill is invalid: {}", skill),
            Some(crate::tr!(
                "スキル名はディレクトリ名のみを指定してください",
                "Use a directory name only."
            )),
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
