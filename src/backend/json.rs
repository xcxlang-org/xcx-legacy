use super::runtime;
use super::tables::{self, TableCell};
use crate::sema::json;
use serde_json::Value;

fn as_json(ptr: i64) -> &'static Value {
    unsafe { &*(ptr as *const Value) }
}

fn read_path(path_ptr: i64, path_len: i64) -> String {
    runtime::read_text(path_ptr, path_len)
}

fn require_at<'a>(value: &'a Value, path: &str) -> &'a Value {
    match json::navigate(value, path) {
        Some(found) => found,
        None => runtime::halt(&format!("path `{path}` does not exist in the json")),
    }
}

fn require_typed<'a>(value: &'a Value, path: &str, expected: &str) -> &'a Value {
    let found = require_at(value, path);
    if found.is_null() {
        runtime::halt(&format!("the value at path `{path}` is null"));
    }
    if json::describe(found) != expected {
        runtime::halt(&format!(
            "the value at path `{path}` is {}, expected {expected}",
            json::describe(found)
        ));
    }
    found
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_parse(text_ptr: i64, text_len: i64) -> i64 {
    let text = runtime::read_text(text_ptr, text_len);
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => super::memory::allocate(value),
        Err(error) => runtime::halt(&format!("invalid json: {error}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_count(json_ptr: i64) -> i64 {
    match as_json(json_ptr) {
        Value::Object(fields) => fields.len() as i64,
        Value::Array(elements) => elements.len() as i64,
        other => runtime::halt(&format!(
            "`count` requires a json object or array, got {}",
            json::describe(other)
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_field(json_ptr: i64, name_ptr: i64, name_len: i64) -> i64 {
    let name = read_path(name_ptr, name_len);
    match as_json(json_ptr) {
        Value::Object(fields) => match fields.get(&name) {
            Some(value) => super::memory::allocate(value.clone()),
            None => runtime::halt(&format!("field `{name}` does not exist in the json")),
        },
        other => runtime::halt(&format!(
            "field access requires a json object, got {}",
            json::describe(other)
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_exists(json_ptr: i64, path_ptr: i64, path_len: i64) -> i8 {
    let path = read_path(path_ptr, path_len);
    i8::from(json::navigate(as_json(json_ptr), &path).is_some())
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_get_int(json_ptr: i64, path_ptr: i64, path_len: i64) -> i64 {
    let path = read_path(path_ptr, path_len);
    let value = require_typed(as_json(json_ptr), &path, "int");
    value.as_i64().unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_get_float(json_ptr: i64, path_ptr: i64, path_len: i64) -> f64 {
    let path = read_path(path_ptr, path_len);
    let value = require_typed(as_json(json_ptr), &path, "float");
    value.as_f64().unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_get_bool(json_ptr: i64, path_ptr: i64, path_len: i64) -> i8 {
    let path = read_path(path_ptr, path_len);
    let value = require_typed(as_json(json_ptr), &path, "boolean");
    i8::from(value.as_bool().unwrap_or(false))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_get_str(
    json_ptr: i64,
    path_ptr: i64,
    path_len: i64,
    out_len: *mut i64,
) -> i64 {
    let path = read_path(path_ptr, path_len);
    let value = require_typed(as_json(json_ptr), &path, "str");
    let text = value.as_str().unwrap_or_default();
    unsafe { *out_len = text.len() as i64 };
    super::memory::allocate_bytes(text.as_bytes().to_vec())
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_to_str(json_ptr: i64, out_len: *mut i64) -> i64 {
    let rendered = serde_json::to_string(as_json(json_ptr)).unwrap_or_default();
    unsafe { *out_len = rendered.len() as i64 };
    super::memory::allocate_bytes(rendered.into_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_print(json_ptr: i64) {
    let rendered = serde_json::to_string(as_json(json_ptr)).unwrap_or_default();
    runtime::write_line(&rendered);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_json_inject(json_ptr: i64, mapping_ptr: i64, table_ptr: i64) {
    let value = as_json(json_ptr);
    let Value::Array(objects) = value else {
        runtime::halt(&format!(
            "`inject` requires a json array, got {}",
            json::describe(value)
        ));
    };
    let mapping = super::maps::text_entries(mapping_ptr);
    if mapping.is_empty() {
        runtime::halt("`inject` requires a non-empty mapping map");
    }
    for object in objects {
        inject_row(object, &mapping, table_ptr);
    }
}

fn inject_row(object: &Value, mapping: &[(String, String)], table_ptr: i64) {
    let Value::Object(fields) = object else {
        runtime::halt(&format!(
            "`inject` requires json objects in the array, got {}",
            json::describe(object)
        ));
    };
    let shape = tables::shape_of_public(table_ptr);
    let mut cells: Vec<TableCell> = Vec::with_capacity(shape.columns.len());
    for (column_name, kind, auto) in &shape.columns {
        if *auto {
            cells.push(TableCell::Int(0));
            continue;
        }
        let Some(source) = mapping.iter().find(|(_, target)| target == column_name) else {
            runtime::halt(&format!(
                "`inject` mapping does not provide a value for column `{column_name}`"
            ));
        };
        let Some(value) = fields.get(&source.0) else {
            runtime::halt(&format!(
                "the field `{}` does not exist in a json object of the injected array",
                source.0
            ));
        };
        cells.push(coerce_cell(value, *kind, &source.0));
    }
    tables::append_prepared_row(table_ptr, cells);
}

fn coerce_cell(value: &Value, kind: i32, field: &str) -> TableCell {
    if value.is_null() {
        runtime::halt(&format!("the value of field `{field}` is null"));
    }
    let mismatch = || {
        runtime::halt(&format!(
            "the value of field `{field}` is {}, which does not fit the column",
            json::describe(value)
        ))
    };
    match kind {
        tables::KIND_INT => match value.as_i64() {
            Some(number) => TableCell::Int(number),
            None => mismatch(),
        },
        tables::KIND_FLOAT => match value.as_f64() {
            Some(number) => TableCell::Float(number),
            None => mismatch(),
        },
        tables::KIND_TEXT => match value.as_str() {
            Some(text) => {
                TableCell::Text(super::memory::allocate_bytes(text.as_bytes().to_vec()), text.len() as i64)
            }
            None => mismatch(),
        },
        tables::KIND_BOOL => match value.as_bool() {
            Some(flag) => TableCell::Bool(flag),
            None => mismatch(),
        },
        _ => runtime::halt("table columns of this type cannot be injected yet"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_read_scalar_paths() {
        let text = "{\"name\":\"John\",\"age\":25,\"score\":91.5,\"active\":true}";
        let json_ptr = xcx_json_parse(text.as_ptr() as i64, text.len() as i64);
        assert_eq!(xcx_json_count(json_ptr), 4);
        let mut len: i64 = 0;
        let name = b"name";
        let ptr = xcx_json_get_str(json_ptr, name.as_ptr() as i64, 4, &mut len);
        assert_eq!(runtime::read_text(ptr, len), "John");
        let age = b"age";
        assert_eq!(xcx_json_get_int(json_ptr, age.as_ptr() as i64, 3), 25);
    }
}
