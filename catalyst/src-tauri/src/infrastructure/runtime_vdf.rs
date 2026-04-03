#[derive(Debug, Clone)]
pub(crate) enum VdfValue {
    Object(Vec<(String, VdfValue)>),
    Text(String),
}

#[derive(Debug, Clone)]
enum VdfToken {
    OpenBrace,
    CloseBrace,
    Text(String),
}

fn tokenize_vdf(contents: &str) -> Vec<VdfToken> {
    let normalized_contents = contents.trim_start_matches('\u{feff}');
    let mut characters = normalized_contents.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(character) = characters.next() {
        match character {
            '{' => tokens.push(VdfToken::OpenBrace),
            '}' => tokens.push(VdfToken::CloseBrace),
            '"' => {
                let mut value = String::new();
                while let Some(inner_character) = characters.next() {
                    if inner_character == '"' {
                        break;
                    }

                    if inner_character == '\0' {
                        continue;
                    }

                    if inner_character == '\\' {
                        if let Some(escaped_character) = characters.next() {
                            match escaped_character {
                                '\\' => value.push('\\'),
                                '"' => value.push('"'),
                                'n' => value.push('\n'),
                                'r' => value.push('\r'),
                                't' => value.push('\t'),
                                '\0' => {}
                                other => value.push(other),
                            }
                        }
                        continue;
                    }

                    value.push(inner_character);
                }
                tokens.push(VdfToken::Text(value));
            }
            '/' => {
                if matches!(characters.peek(), Some('/')) {
                    let _ = characters.next();
                    for comment_character in characters.by_ref() {
                        if comment_character == '\n' {
                            break;
                        }
                    }
                    continue;
                }

                let mut bare_token = String::from('/');
                while let Some(peeked_character) = characters.peek().copied() {
                    if peeked_character == '\0' {
                        let _ = characters.next();
                        continue;
                    }
                    if peeked_character.is_whitespace()
                        || peeked_character == '{'
                        || peeked_character == '}'
                    {
                        break;
                    }
                    bare_token.push(peeked_character);
                    let _ = characters.next();
                }
                tokens.push(VdfToken::Text(bare_token));
            }
            value if value.is_whitespace() || value == '\0' => {}
            value => {
                let mut bare_token = String::new();
                bare_token.push(value);
                while let Some(peeked_character) = characters.peek().copied() {
                    if peeked_character == '\0' {
                        let _ = characters.next();
                        continue;
                    }
                    if peeked_character.is_whitespace()
                        || peeked_character == '{'
                        || peeked_character == '}'
                    {
                        break;
                    }
                    bare_token.push(peeked_character);
                    let _ = characters.next();
                }
                tokens.push(VdfToken::Text(bare_token));
            }
        }
    }

    tokens
}

fn parse_vdf_tokens(tokens: &[VdfToken], cursor: &mut usize) -> Result<Vec<(String, VdfValue)>, String> {
    let mut entries = Vec::new();

    while *cursor < tokens.len() {
        let Some(token) = tokens.get(*cursor) else {
            break;
        };

        match token {
            VdfToken::CloseBrace => {
                *cursor += 1;
                break;
            }
            VdfToken::OpenBrace => {
                return Err(String::from("Invalid VDF format: unexpected '{'"));
            }
            VdfToken::Text(key) => {
                let key = key.clone();
                *cursor += 1;
                let Some(value_token) = tokens.get(*cursor) else {
                    return Err(format!(
                        "Invalid VDF format: missing value for key '{key}'"
                    ));
                };

                match value_token {
                    VdfToken::Text(value) => {
                        entries.push((key, VdfValue::Text(value.clone())));
                        *cursor += 1;
                    }
                    VdfToken::OpenBrace => {
                        *cursor += 1;
                        let object_value = parse_vdf_tokens(tokens, cursor)?;
                        entries.push((key, VdfValue::Object(object_value)));
                    }
                    VdfToken::CloseBrace => {
                        return Err(format!(
                            "Invalid VDF format: missing value for key '{key}'"
                        ));
                    }
                }
            }
        }
    }

    Ok(entries)
}

pub(crate) fn parse_vdf_document(contents: &str) -> Result<VdfValue, String> {
    let tokens = tokenize_vdf(contents);
    let mut cursor = 0;
    let entries = parse_vdf_tokens(&tokens, &mut cursor)?;
    if cursor < tokens.len() {
        return Err(String::from("Invalid VDF format: trailing tokens"));
    }
    Ok(VdfValue::Object(entries))
}

pub(crate) fn vdf_find_object_value<'a>(value: &'a VdfValue, key: &str) -> Option<&'a VdfValue> {
    let VdfValue::Object(entries) = value else {
        return None;
    };

    entries
        .iter()
        .find(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
        .map(|(_, entry_value)| entry_value)
}

pub(crate) fn vdf_collect_objects_by_key<'a>(
    value: &'a VdfValue,
    key: &str,
    output: &mut Vec<&'a VdfValue>,
) {
    let VdfValue::Object(entries) = value else {
        return;
    };

    for (entry_key, entry_value) in entries {
        if entry_key.eq_ignore_ascii_case(key) && matches!(entry_value, VdfValue::Object(_)) {
            output.push(entry_value);
        }
        vdf_collect_objects_by_key(entry_value, key, output);
    }
}

fn vdf_get_or_insert_object_mut<'a>(value: &'a mut VdfValue, key: &str) -> &'a mut VdfValue {
    if matches!(value, VdfValue::Text(_)) {
        *value = VdfValue::Object(Vec::new());
    }

    let VdfValue::Object(entries) = value else {
        unreachable!()
    };

    if let Some(entry_index) = entries
        .iter()
        .position(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
    {
        if !matches!(entries[entry_index].1, VdfValue::Object(_)) {
            entries[entry_index].1 = VdfValue::Object(Vec::new());
        }
        return &mut entries[entry_index].1;
    }

    entries.push((key.to_owned(), VdfValue::Object(Vec::new())));
    let last_index = entries.len() - 1;
    &mut entries[last_index].1
}

pub(crate) fn vdf_ensure_object_path_mut<'a>(
    value: &'a mut VdfValue,
    path: &[&str],
) -> &'a mut VdfValue {
    if path.is_empty() {
        return value;
    }

    let child = vdf_get_or_insert_object_mut(value, path[0]);
    vdf_ensure_object_path_mut(child, &path[1..])
}

pub(crate) fn vdf_set_text_entry(value: &mut VdfValue, key: &str, text: &str) {
    if matches!(value, VdfValue::Text(_)) {
        *value = VdfValue::Object(Vec::new());
    }

    let VdfValue::Object(entries) = value else {
        unreachable!()
    };
    if let Some(entry_index) = entries
        .iter()
        .position(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
    {
        entries[entry_index].1 = VdfValue::Text(text.to_owned());
        return;
    }

    entries.push((key.to_owned(), VdfValue::Text(text.to_owned())));
}

pub(crate) fn vdf_remove_entry(value: &mut VdfValue, key: &str) {
    let VdfValue::Object(entries) = value else {
        return;
    };
    entries.retain(|(entry_key, _)| !entry_key.eq_ignore_ascii_case(key));
}

pub(crate) fn vdf_get_text_entry<'a>(value: &'a VdfValue, key: &str) -> Option<&'a str> {
    let VdfValue::Object(entries) = value else {
        return None;
    };
    entries
        .iter()
        .find(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
        .and_then(|(_, entry_value)| match entry_value {
            VdfValue::Text(text) => Some(text.as_str()),
            VdfValue::Object(_) => None,
        })
}

fn escape_vdf_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }

    escaped
}

fn serialize_vdf_entry(key: &str, value: &VdfValue, depth: usize, output: &mut String) {
    let indent = "\t".repeat(depth);
    output.push_str(&indent);
    output.push('"');
    output.push_str(&escape_vdf_text(key));
    output.push('"');

    match value {
        VdfValue::Text(text) => {
            output.push('\t');
            output.push('"');
            output.push_str(&escape_vdf_text(text));
            output.push('"');
            output.push('\n');
        }
        VdfValue::Object(entries) => {
            output.push('\n');
            output.push_str(&indent);
            output.push_str("{\n");
            for (entry_key, entry_value) in entries {
                serialize_vdf_entry(entry_key, entry_value, depth + 1, output);
            }
            output.push_str(&indent);
            output.push_str("}\n");
        }
    }
}

pub(crate) fn serialize_vdf_document(value: &VdfValue) -> String {
    let mut output = String::new();
    match value {
        VdfValue::Object(entries) => {
            for (entry_key, entry_value) in entries {
                serialize_vdf_entry(entry_key, entry_value, 0, &mut output);
            }
        }
        VdfValue::Text(text) => {
            output.push('"');
            output.push_str(&escape_vdf_text(text));
            output.push('"');
            output.push('\n');
        }
    }

    output
}

pub(crate) fn vdf_collect_text_leaves(value: &VdfValue, output: &mut Vec<String>) {
    match value {
        VdfValue::Text(text) => output.push(text.clone()),
        VdfValue::Object(entries) => {
            for (_, entry_value) in entries {
                vdf_collect_text_leaves(entry_value, output);
            }
        }
    }
}
