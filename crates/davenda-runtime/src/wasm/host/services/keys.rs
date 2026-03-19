pub(super) fn env_key_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut last_was_underscore = false;

    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_uppercase()
        } else {
            '_'
        };

        if mapped == '_' {
            if last_was_underscore {
                continue;
            }
            last_was_underscore = true;
        } else {
            last_was_underscore = false;
        }

        sanitized.push(mapped);
    }

    while sanitized.starts_with('_') {
        sanitized.remove(0);
    }
    while sanitized.ends_with('_') {
        sanitized.pop();
    }

    if sanitized.is_empty() {
        "DEFAULT".to_string()
    } else {
        sanitized
    }
}
