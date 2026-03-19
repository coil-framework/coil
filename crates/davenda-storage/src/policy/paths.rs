use super::StoragePolicyError;

pub(crate) fn normalize_relative_path(input: &str) -> Result<String, StoragePolicyError> {
    let trimmed = input.trim();

    if trimmed.is_empty() || trimmed.starts_with('/') {
        return Err(StoragePolicyError::InvalidRelativePath {
            path: input.to_string(),
        });
    }

    let mut segments = Vec::new();
    for segment in trimmed.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                return Err(StoragePolicyError::ParentTraversal {
                    path: input.to_string(),
                });
            }
            _ => segments.push(segment),
        }
    }

    if segments.is_empty() {
        return Err(StoragePolicyError::InvalidRelativePath {
            path: input.to_string(),
        });
    }

    Ok(segments.join("/"))
}

pub(crate) fn normalize_rule_prefix(input: &str) -> Result<String, StoragePolicyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    normalize_relative_path(trimmed)
}

pub(crate) fn join_relative(prefix: Option<&str>, logical_path: &str) -> String {
    let mut parts = Vec::new();

    if let Some(prefix) = prefix {
        let normalized = prefix.trim().trim_matches('/');
        if !normalized.is_empty() {
            parts.push(normalized);
        }
    }

    let normalized_path = logical_path.trim().trim_matches('/');
    if !normalized_path.is_empty() {
        parts.push(normalized_path);
    }

    parts.join("/")
}

pub(crate) fn join_local_path(root: &str, subdir: Option<&str>, logical_path: &str) -> String {
    let root = root.trim_end_matches('/');
    let root_is_absolute = root.starts_with('/');
    let mut parts = Vec::new();

    if !root.is_empty() {
        parts.push(if root_is_absolute {
            root.trim_start_matches('/').to_string()
        } else {
            root.to_string()
        });
    }

    if let Some(subdir) = subdir {
        let normalized = subdir.trim().trim_matches('/');
        if !normalized.is_empty() {
            parts.push(normalized.to_string());
        }
    }

    let normalized_path = logical_path.trim().trim_matches('/');
    if !normalized_path.is_empty() {
        parts.push(normalized_path.to_string());
    }

    let joined = parts.join("/");
    if root_is_absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

pub(crate) fn trim_trailing_separator(input: &str) -> String {
    let trimmed = input.trim_end_matches('/');
    if trimmed.is_empty() {
        input.to_string()
    } else {
        trimmed.to_string()
    }
}
