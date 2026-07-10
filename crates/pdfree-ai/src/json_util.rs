//! Small shared helper for parsing a JSON object out of a raw model
//! response — used by every feature that asks a [`crate::provider::Provider`]
//! for structured output (`formfill`, `schema_extract`), since local models
//! in particular tend to wrap JSON in prose or markdown code fences despite
//! being asked not to.

/// Find the first top-level `{...}` object in `text`.
pub fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    for (i, c) in text[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + i + c.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_first_balanced_braces() {
        let text =
            "Sure, here's the mapping:\n```json\n{\"FullName\": \"Jane Doe\"}\n```\nLet me know!";
        assert_eq!(
            extract_json_object(text),
            Some("{\"FullName\": \"Jane Doe\"}")
        );
    }

    #[test]
    fn handles_nested_braces() {
        let text = "{\"a\": {\"nested\": true}, \"b\": 1}";
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn returns_none_with_no_braces_at_all() {
        assert_eq!(extract_json_object("no json here"), None);
    }
}
