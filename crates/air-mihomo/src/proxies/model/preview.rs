use serde_yaml::Value;

pub(super) fn yaml_value_preview(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Sequence(values) => format!("[{} items]", values.len()),
        Value::Mapping(values) => format!("{{{} fields}}", values.len()),
        Value::Tagged(tagged) => yaml_value_preview(&tagged.value),
    }
}

pub(super) fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
