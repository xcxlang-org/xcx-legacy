use serde_json::Value;

pub fn navigate<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        let Value::Object(fields) = current else {
            return None;
        };
        current = fields.get(segment)?;
    }
    Some(current)
}

pub fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() => "int",
        Value::Number(_) => "float",
        Value::String(_) => "str",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn matches_type(value: &Value, expected: &crate::sema::types::SemaType) -> bool {
    use crate::sema::types::SemaType;
    match (value, expected) {
        (Value::Bool(_), SemaType::Bool) => true,
        (Value::Number(number), SemaType::Int) => number.is_i64(),
        (Value::Number(_), SemaType::Float) => true,
        (Value::String(_), SemaType::Str) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn navigation_walks_dot_paths() {
        let value = json!({"user": {"name": "John"}});
        assert_eq!(navigate(&value, "user.name"), Some(&json!("John")));
        assert_eq!(navigate(&value, "user.missing"), None);
        assert_eq!(navigate(&value, "nope.name"), None);
    }

    #[test]
    fn values_describe_as_xcx_types() {
        assert_eq!(describe(&json!(25)), "int");
        assert_eq!(describe(&json!(2.5)), "float");
        assert_eq!(describe(&json!(true)), "boolean");
        assert_eq!(describe(&json!("x")), "str");
        assert_eq!(describe(&Value::Null), "null");
    }
}
