use std::collections::HashMap;
use std::path::Path;

use rayon::prelude::*;

use crate::models::api::PathValidationResult;

#[tauri::command]
pub fn validate_path(
    relative_path: String,
    project_path: String,
) -> Result<PathValidationResult, String> {
    let base = Path::new(&project_path);
    let target = base.join(&relative_path);

    Ok(PathValidationResult {
        exists: target.exists(),
        is_directory: if target.exists() {
            Some(target.is_dir())
        } else {
            None
        },
    })
}

#[tauri::command]
pub fn validate_mentions(
    mentions: Vec<MentionInput>,
    project_path: String,
) -> Result<HashMap<String, bool>, String> {
    let base = Path::new(&project_path);

    let results: HashMap<String, bool> = mentions
        .par_iter()
        .map(|mention| {
            let target = base.join(&mention.value);
            (mention.value.clone(), target.exists())
        })
        .collect();

    Ok(results)
}

/// Input for a mention validation request.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MentionInput {
    #[serde(rename = "type")]
    pub mention_type: String, // "path"
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_existing_directory() {
        let result = validate_path("tmp".to_string(), "/".to_string()).unwrap();
        assert!(result.exists);
        assert_eq!(result.is_directory, Some(true));
    }

    #[test]
    fn test_validate_path_nonexistent() {
        let result =
            validate_path("nonexistent_xyz_123".to_string(), "/tmp".to_string()).unwrap();
        assert!(!result.exists);
        assert!(result.is_directory.is_none());
    }

    #[test]
    fn test_validate_mentions_empty() {
        let result = validate_mentions(vec![], "/tmp".to_string()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_validate_mentions_with_paths() {
        let mentions = vec![
            MentionInput {
                mention_type: "path".to_string(),
                value: "tmp".to_string(),
            },
            MentionInput {
                mention_type: "path".to_string(),
                value: "nonexistent_xyz_123".to_string(),
            },
        ];
        let result = validate_mentions(mentions, "/".to_string()).unwrap();
        assert_eq!(result.get("tmp"), Some(&true));
        assert_eq!(result.get("nonexistent_xyz_123"), Some(&false));
    }
}
