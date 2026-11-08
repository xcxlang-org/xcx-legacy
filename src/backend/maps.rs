use super::runtime;
use super::memory::{self, Managed};


#[derive(Clone, Copy, Debug)]
pub enum MapKey {
    Int(i64),
    Text(i64, i64),
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MapKey::Int(a), MapKey::Int(b)) => a == b,
            (MapKey::Text(ap, al), MapKey::Text(bp, bl)) => {
                al == bl && text_bytes(*ap, *al) == text_bytes(*bp, *bl)
            }
            _ => false,
        }
    }
}

fn text_bytes(ptr: i64, len: i64) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) }.to_vec()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MapValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Character(char),
    Text(i64, i64),
}

pub struct MapObject {
    entries: Vec<(MapKey, MapValue)>,
}

pub struct KeysList {
    entries: Vec<MapKey>,
}

impl MapKey {
    fn trace(&self, visit: &mut dyn FnMut(i64)) {
        if let Self::Text(pointer, _) = self {
            visit(*pointer);
        }
    }
}

impl Managed for MapObject {
    fn trace(&self, visit: &mut dyn FnMut(i64)) {
        for (key, value) in &self.entries {
            key.trace(visit);
            if let MapValue::Text(pointer, _) = value {
                visit(*pointer);
            }
        }
    }
}

impl Managed for KeysList {
    fn trace(&self, visit: &mut dyn FnMut(i64)) {
        for key in &self.entries {
            key.trace(visit);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Int,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Int,
    Float,
    Bool,
    Character,
    Text,
}

fn decode_key(kind: KeyKind, a: i64, b: i64) -> MapKey {
    match kind {
        KeyKind::Int => MapKey::Int(a),
        KeyKind::Text => MapKey::Text(a, b),
    }
}

fn decode_value(kind: ValueKind, a: i64, b: i64, f: f64) -> MapValue {
    match kind {
        ValueKind::Int => MapValue::Int(a),
        ValueKind::Float => MapValue::Float(f),
        ValueKind::Bool => MapValue::Bool(a != 0),
        ValueKind::Character => MapValue::Character(char_from_code(a)),
        ValueKind::Text => MapValue::Text(a, b),
    }
}

fn char_from_code(code: i64) -> char {
    char::from_u32(code as u32).unwrap_or('\u{FFFD}')
}

fn as_map(ptr: i64) -> &'static MapObject {
    unsafe { &*(ptr as *const MapObject) }
}

fn as_map_mut(ptr: i64) -> &'static mut MapObject {
    unsafe { &mut *(ptr as *mut MapObject) }
}

fn insert_entry(map: i64, key: MapKey, value: MapValue) {
    let object = as_map_mut(map);
    for entry in &mut object.entries {
        if entry.0 == key {
            entry.1 = value;
            return;
        }
    }
    object.entries.push((key, value));
}

fn lookup(map: i64, key: &MapKey) -> Option<MapValue> {
    as_map(map)
        .entries
        .iter()
        .find(|entry| &entry.0 == key)
        .map(|entry| entry.1)
}

fn contains_key(map: i64, key: &MapKey) -> bool {
    as_map(map).entries.iter().any(|entry| &entry.0 == key)
}

fn key_display(key: &MapKey) -> String {
    match key {
        MapKey::Int(value) => value.to_string(),
        MapKey::Text(ptr, len) => {
            let bytes = unsafe { std::slice::from_raw_parts(*ptr as *const u8, *len as usize) };
            format!("\"{}\"", String::from_utf8_lossy(bytes))
        }
    }
}

fn value_display(value: &MapValue) -> String {
    match value {
        MapValue::Int(value) => value.to_string(),
        MapValue::Float(value) => runtime::format_float(*value),
        MapValue::Bool(value) => value.to_string(),
        MapValue::Character(value) => value.to_string(),
        MapValue::Text(ptr, len) => {
            let bytes = unsafe { std::slice::from_raw_parts(*ptr as *const u8, *len as usize) };
            format!("\"{}\"", String::from_utf8_lossy(bytes))
        }
    }
}

macro_rules! map_pair {
    ($kind_key:expr, $kind_value:expr, $insert:ident, $get:ident, $get_return:ty, $unwrap:pat, $convert:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $insert(
            map: i64,
            key_a: i64,
            key_b: i64,
            value_a: i64,
            value_b: i64,
            value_f: f64,
        ) {
            let key = decode_key($kind_key, key_a, key_b);
            let value = decode_value($kind_value, value_a, value_b, value_f);
            insert_entry(map, key, value);
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $get(map: i64, key_a: i64, key_b: i64, out_len: *mut i64) -> $get_return {
            let _ = out_len;
            let key = decode_key($kind_key, key_a, key_b);
            match lookup(map, &key) {
                Some(value) => match value {
                    $unwrap => $convert,
                    _ => runtime::halt(&format!(
                        "map value for key {} has an unexpected type",
                        key_display(&key)
                    )),
                },
                None => runtime::halt(&format!(
                    "key {} does not exist in map",
                    key_display(&key)
                )),
            }
        }
    };
}

map_pair!(KeyKind::Int, ValueKind::Int, xcx_map_ii_insert, xcx_map_ii_get, i64, MapValue::Int(v), v);
map_pair!(KeyKind::Int, ValueKind::Float, xcx_map_if_insert, xcx_map_if_get, f64, MapValue::Float(v), v);
map_pair!(KeyKind::Int, ValueKind::Bool, xcx_map_ib_insert, xcx_map_ib_get, i8, MapValue::Bool(v), i8::from(v));
map_pair!(KeyKind::Int, ValueKind::Character, xcx_map_ic_insert, xcx_map_ic_get, i32, MapValue::Character(v), v as i32);
map_pair!(KeyKind::Text, ValueKind::Int, xcx_map_si_insert, xcx_map_si_get, i64, MapValue::Int(v), v);
map_pair!(KeyKind::Text, ValueKind::Float, xcx_map_sf_insert, xcx_map_sf_get, f64, MapValue::Float(v), v);
map_pair!(KeyKind::Text, ValueKind::Bool, xcx_map_sb_insert, xcx_map_sb_get, i8, MapValue::Bool(v), i8::from(v));
map_pair!(KeyKind::Text, ValueKind::Character, xcx_map_sc_insert, xcx_map_sc_get, i32, MapValue::Character(v), v as i32);

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_is_insert(map: i64, key_a: i64, key_b: i64, value_a: i64, value_b: i64, value_f: f64) {
    let key = decode_key(KeyKind::Int, key_a, key_b);
    let value = decode_value(ValueKind::Text, value_a, value_b, value_f);
    insert_entry(map, key, value);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_is_get(map: i64, key_a: i64, key_b: i64, out_len: *mut i64) -> i64 {
    let key = decode_key(KeyKind::Int, key_a, key_b);
    text_get(map, &key, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_ss_insert(map: i64, key_a: i64, key_b: i64, value_a: i64, value_b: i64, value_f: f64) {
    let key = decode_key(KeyKind::Text, key_a, key_b);
    let value = decode_value(ValueKind::Text, value_a, value_b, value_f);
    insert_entry(map, key, value);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_ss_get(map: i64, key_a: i64, key_b: i64, out_len: *mut i64) -> i64 {
    let key = decode_key(KeyKind::Text, key_a, key_b);
    text_get(map, &key, out_len)
}

fn text_get(map: i64, key: &MapKey, out_len: *mut i64) -> i64 {
    match lookup(map, key) {
        Some(MapValue::Text(ptr, len)) => {
            unsafe { *out_len = len };
            ptr
        }
        Some(_) => runtime::halt(&format!(
            "map value for key {} has an unexpected type",
            key_display(key)
        )),
        None => runtime::halt(&format!(
            "key {} does not exist in map",
            key_display(key)
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_new() -> i64 {
    memory::allocate(MapObject { entries: Vec::new() })
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_i_has(map: i64, key: i64) -> i8 {
    i8::from(contains_key(map, &MapKey::Int(key)))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_s_has(map: i64, key_a: i64, key_b: i64) -> i8 {
    i8::from(contains_key(map, &MapKey::Text(key_a, key_b)))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_i_keys(map: i64) -> i64 {
    keys_list(map, KeyKind::Int)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_s_keys(map: i64) -> i64 {
    keys_list(map, KeyKind::Text)
}

fn keys_list(map: i64, kind: KeyKind) -> i64 {
    let entries: Vec<MapKey> = as_map(map)
        .entries
        .iter()
        .map(|(key, _)| match (kind, key) {
            (KeyKind::Int, MapKey::Int(value)) => MapKey::Int(*value),
            (KeyKind::Text, MapKey::Text(ptr, len)) => MapKey::Text(*ptr, *len),
            _ => unreachable!("keys() requested for the declared key kind"),
        })
        .collect();
    memory::allocate(KeysList { entries })
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_keys_len(list: i64) -> i64 {
    unsafe { &*(list as *mut KeysList) }.entries.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_keys_get_int(list: i64, index: i64) -> i64 {
    let list = unsafe { &*(list as *mut KeysList) };
    match list.entries.get(index as usize) {
        Some(MapKey::Int(value)) => *value,
        _ => runtime::halt("map key iteration out of bounds"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_keys_get_str(list: i64, index: i64, out_len: *mut i64) -> i64 {
    let list = unsafe { &*(list as *mut KeysList) };
    match list.entries.get(index as usize) {
        Some(MapKey::Text(ptr, len)) => {
            unsafe { *out_len = *len };
            *ptr
        }
        _ => runtime::halt("map key iteration out of bounds"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_size(map: i64) -> i64 {
    as_map(map).entries.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_map_show(map: i64) {
    let object = as_map(map);
    let rendered: Vec<String> = object
        .entries
        .iter()
        .map(|(key, value)| format!("{} :: {}", key_display(key), value_display(value)))
        .collect();
    runtime::write_line(&format!("{{ {} }}", rendered.join(", ")));
}

pub(crate) fn text_entries(map: i64) -> Vec<(String, String)> {
    as_map(map)
        .entries
        .iter()
        .filter_map(|(key, value)| match (key, value) {
            (MapKey::Text(ptr, len), MapValue::Text(value_ptr, value_len)) => Some((
                runtime::read_text(*ptr, *len),
                runtime::read_text(*value_ptr, *value_len),
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_updates_existing_keys_and_reports_size() {
        let map = xcx_map_new();
        xcx_map_ii_insert(map, 1, 0, 10, 0, 0.0);
        xcx_map_ii_insert(map, 2, 0, 20, 0, 0.0);
        xcx_map_ii_insert(map, 1, 0, 11, 0, 0.0);
        assert_eq!(xcx_map_size(map), 2);
        let mut out_len: i64 = 0;
        assert_eq!(xcx_map_ii_get(map, 1, 0, &mut out_len), 11);
        assert_eq!(xcx_map_i_has(map, 2), 1);
        assert_eq!(xcx_map_i_has(map, 3), 0);
    }

    #[test]
    fn text_keys_and_text_values_round_trip() {
        let left = b"John";
        let map = xcx_map_new();
        xcx_map_ss_insert(map, left.as_ptr() as i64, 4, left.as_ptr() as i64, 4, 0.0);
        let mut out_len: i64 = 0;
        let ptr = xcx_map_ss_get(map, left.as_ptr() as i64, 4, &mut out_len);
        assert_eq!(out_len, 4);
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, out_len as usize) };
        assert_eq!(bytes, left);
        assert_eq!(xcx_map_s_has(map, left.as_ptr() as i64, 4), 1);
    }

    #[test]
    fn keys_list_preserves_insertion_order() {
        let first = b"a";
        let second = b"b";
        let map = xcx_map_new();
        xcx_map_si_insert(map, first.as_ptr() as i64, 1, 1, 0, 0.0);
        xcx_map_si_insert(map, second.as_ptr() as i64, 1, 2, 0, 0.0);
        let list = xcx_map_s_keys(map);
        assert_eq!(xcx_keys_len(list), 2);
        let mut out_len: i64 = 0;
        assert_eq!(xcx_keys_get_str(list, 0, &mut out_len), first.as_ptr() as i64);
        assert_eq!(out_len, 1);
        assert_eq!(xcx_keys_get_str(list, 1, &mut out_len), second.as_ptr() as i64);
    }
}
