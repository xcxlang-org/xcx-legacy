use std::sync::atomic::{AtomicU64, Ordering};

use super::runtime;
use super::memory::{self, Managed};

pub const DESIGNATOR_NATURAL: i32 = 0;
pub const DESIGNATOR_INTEGER: i32 = 1;
pub const DESIGNATOR_RATIONAL: i32 = 2;
pub const DESIGNATOR_TEXT: i32 = 3;
pub const DESIGNATOR_BOOLEAN: i32 = 4;
pub const DESIGNATOR_CHARACTER: i32 = 5;


#[derive(Clone, Debug)]
pub enum SetElement {
    Int(i64),
    Float(f64),
    Text(i64, i64),
    Bool(bool),
    Character(char),
}

impl PartialEq for SetElement {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SetElement::Int(a), SetElement::Int(b)) => a == b,
            (SetElement::Float(a), SetElement::Float(b)) => a == b,
            (SetElement::Bool(a), SetElement::Bool(b)) => a == b,
            (SetElement::Character(a), SetElement::Character(b)) => a == b,
            (SetElement::Text(ap, al), SetElement::Text(bp, bl)) => {
                al == bl
                    && unsafe { std::slice::from_raw_parts(*ap as *const u8, *al as usize) }
                        == unsafe { std::slice::from_raw_parts(*bp as *const u8, *bl as usize) }
            }
            _ => false,
        }
    }
}

pub struct SetObject {
    designator: i32,
    elements: Vec<SetElement>,
}

pub struct SetBuilder {
    designator: i32,
    elements: Vec<SetElement>,
}

impl Managed for SetObject {
    fn trace(&self, visit: &mut dyn FnMut(i64)) {
        for element in &self.elements {
            if let SetElement::Text(pointer, _) = element {
                visit(*pointer);
            }
        }
    }
}

impl Managed for SetBuilder {
    fn trace(&self, visit: &mut dyn FnMut(i64)) {
        for element in &self.elements {
            if let SetElement::Text(pointer, _) = element {
                visit(*pointer);
            }
        }
    }
}

fn designator_name(designator: i32) -> &'static str {
    match designator {
        DESIGNATOR_NATURAL => "N",
        DESIGNATOR_INTEGER => "Z",
        DESIGNATOR_RATIONAL => "Q",
        DESIGNATOR_TEXT => "S",
        DESIGNATOR_BOOLEAN => "B",
        DESIGNATOR_CHARACTER => "C",
        _ => "?",
    }
}

fn random_state() -> &'static AtomicU64 {
    static STATE: AtomicU64 = AtomicU64::new(0);
    &STATE
}

fn next_random() -> u64 {
    let state = random_state();
    let mut current = state.load(Ordering::Relaxed);
    if current == 0 {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        current = seed | 1;
    }
    current ^= current << 13;
    current ^= current >> 7;
    current ^= current << 17;
    state.store(current, Ordering::Relaxed);
    current
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_builder_new(designator: i32) -> i64 {
    memory::allocate(SetBuilder { designator, elements: Vec::new() })
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_int(builder_ptr: i64, value: i64) {
    push_element(builder_ptr, SetElement::Int(value));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_float(builder_ptr: i64, value: f64) {
    push_element(builder_ptr, SetElement::Float(value));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_str(builder_ptr: i64, ptr: i64, len: i64) {
    push_element(builder_ptr, make_text(ptr, len));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_bool(builder_ptr: i64, value: i8) {
    push_element(builder_ptr, SetElement::Bool(value != 0));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_char(builder_ptr: i64, value: i32) {
    push_element(builder_ptr, SetElement::Character(make_char(value)));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_range_int(builder_ptr: i64, start: i64, end: i64, step: i64) {
    let step = if step == 0 { 1 } else { step };
    let builder = unsafe { &mut *(builder_ptr as *mut SetBuilder) };
    if step > 0 {
        let mut value = start;
        while value <= end {
            builder.elements.push(SetElement::Int(value));
            value += step;
        }
    } else {
        let mut value = start;
        while value >= end {
            builder.elements.push(SetElement::Int(value));
            value += step;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_add_range_float(builder_ptr: i64, start: f64, end: f64, step: f64) {
    let step = if step == 0.0 { 1.0 } else { step };
    let builder = unsafe { &mut *(builder_ptr as *mut SetBuilder) };
    if step > 0.0 {
        let mut value = start;
        while value <= end {
            builder.elements.push(SetElement::Float(value));
            value += step;
        }
    } else {
        let mut value = start;
        while value >= end {
            builder.elements.push(SetElement::Float(value));
            value += step;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_builder_finish(builder_ptr: i64) -> i64 {
    let builder = unsafe { &mut *(builder_ptr as *mut SetBuilder) };
    memory::allocate(SetObject {
        designator: builder.designator,
        elements: std::mem::take(&mut builder.elements),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_union(left: i64, right: i64) -> i64 {
    let (left_set, right_set) = guarded(left, right);
    let mut elements = left_set.elements.clone();
    for element in &right_set.elements {
        if !elements.contains(element) {
            elements.push(element.clone());
        }
    }
    finish_set(left_set.designator, elements)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_intersection(left: i64, right: i64) -> i64 {
    let (left_set, right_set) = guarded(left, right);
    let elements: Vec<SetElement> = left_set
        .elements
        .iter()
        .filter(|element| right_set.elements.contains(element))
        .cloned()
        .collect();
    finish_set(left_set.designator, elements)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_difference(left: i64, right: i64) -> i64 {
    let (left_set, right_set) = guarded(left, right);
    let elements: Vec<SetElement> = left_set
        .elements
        .iter()
        .filter(|element| !right_set.elements.contains(element))
        .cloned()
        .collect();
    finish_set(left_set.designator, elements)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_symmetric_difference(left: i64, right: i64) -> i64 {
    let (left_set, right_set) = guarded(left, right);
    let mut elements: Vec<SetElement> = left_set
        .elements
        .iter()
        .filter(|element| !right_set.elements.contains(element))
        .cloned()
        .collect();
    for element in &right_set.elements {
        if !left_set.elements.contains(element) && !elements.contains(element) {
            elements.push(element.clone());
        }
    }
    finish_set(left_set.designator, elements)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_is_empty(set: i64) -> i8 {
    let object = unsafe { &*(set as *mut SetObject) };
    i8::from(object.elements.is_empty())
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_has_int(set: i64, value: i64) -> i8 {
    has(set, SetElement::Int(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_has_float(set: i64, value: f64) -> i8 {
    has(set, SetElement::Float(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_has_str(set: i64, ptr: i64, len: i64) -> i8 {
    has(set, make_text(ptr, len))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_has_bool(set: i64, value: i8) -> i8 {
    has(set, SetElement::Bool(value != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_set_has_char(set: i64, value: i32) -> i8 {
    has(set, SetElement::Character(make_char(value)))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_set(set: i64) {
    let object = unsafe { &*(set as *mut SetObject) };
    runtime::write_line(&format_set(object));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_choice_int(set: i64) -> i64 {
    match choose(set) {
        SetElement::Int(value) => value,
        SetElement::Float(value) => value as i64,
        _ => runtime::halt("random.choice: the set does not contain integers"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_choice_float(set: i64) -> f64 {
    match choose(set) {
        SetElement::Float(value) => value,
        SetElement::Int(value) => value as f64,
        _ => runtime::halt("random.choice: the set does not contain floats"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_choice_bool(set: i64) -> i8 {
    match choose(set) {
        SetElement::Bool(value) => i8::from(value),
        _ => runtime::halt("random.choice: the set does not contain booleans"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_choice_char(set: i64) -> i32 {
    match choose(set) {
        SetElement::Character(value) => value as i32,
        _ => runtime::halt("random.choice: the set does not contain characters"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_choice_str(set: i64, out_len: *mut i64) -> i64 {
    match choose(set) {
        SetElement::Text(ptr, len) => {
            unsafe { *out_len = len };
            ptr
        }
        _ => runtime::halt("random.choice: the set does not contain strings"),
    }
}

fn make_text(ptr: i64, len: i64) -> SetElement {
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            SetElement::Text(memory::allocate_bytes(text.as_bytes().to_vec()), len)
        }
        Err(_) => runtime::halt("string with invalid UTF-8 used as a set element"),
    }
}

fn make_char(value: i32) -> char {
    char::from_u32(value as u32).unwrap_or('\u{FFFD}')
}

fn push_element(builder_ptr: i64, element: SetElement) {
    let builder = unsafe { &mut *(builder_ptr as *mut SetBuilder) };
    builder.elements.push(element);
}

fn as_set(ptr: i64) -> &'static SetObject {
    unsafe { &*(ptr as *mut SetObject) }
}

fn contains(set: i64, element: &SetElement) -> bool {
    as_set(set).elements.iter().any(|candidate| candidate == element)
}

fn has(set: i64, element: SetElement) -> i8 {
    i8::from(contains(set, &element))
}

fn guarded(left: i64, right: i64) -> (&'static SetObject, &'static SetObject) {
    let left_set = as_set(left);
    let right_set = as_set(right);
    if left_set.designator != right_set.designator {
        runtime::halt(&format!(
            "set operation on different set types ({} and {})",
            designator_name(left_set.designator),
            designator_name(right_set.designator)
        ));
    }
    (left_set, right_set)
}

fn finish_set(designator: i32, elements: Vec<SetElement>) -> i64 {
    memory::allocate(SetObject { designator, elements })
}

fn choose(set: i64) -> SetElement {
    let object = as_set(set);
    if object.elements.is_empty() {
        runtime::halt("random.choice on an empty set");
    }
    let index = (next_random() % object.elements.len() as u64) as usize;
    object.elements[index].clone()
}

fn format_set(object: &SetObject) -> String {
    let mut text = format!("SetObject(SET_{}: [", designator_name(object.designator));
    let numeric = object
        .elements
        .iter()
        .all(|element| matches!(element, SetElement::Int(_) | SetElement::Float(_)));
    if numeric && !object.elements.is_empty() {
        let numbers: Vec<f64> = object
            .elements
            .iter()
            .map(|element| match element {
                SetElement::Int(value) => *value as f64,
                SetElement::Float(value) => *value,
                _ => unreachable!("checked above"),
            })
            .collect();
        text.push_str(&format_runs(&numbers));
    } else {
        let rendered: Vec<String> = object
            .elements
            .iter()
            .map(|element| match element {
                SetElement::Int(value) => value.to_string(),
                SetElement::Float(value) => super::runtime::format_float(*value),
                SetElement::Text(ptr, len) => {
                    let bytes = unsafe { std::slice::from_raw_parts(*ptr as *const u8, *len as usize) };
                    String::from_utf8_lossy(bytes).into_owned()
                }
                SetElement::Bool(value) => value.to_string(),
                SetElement::Character(value) => value.to_string(),
            })
            .collect();
        text.push_str(&rendered.join(", "));
    }
    text.push(']');
    text.push(')');
    text
}

fn format_runs(numbers: &[f64]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut index = 0usize;
    while index < numbers.len() {
        let start = numbers[index];
        let mut step = 0.0f64;
        let mut last = start;
        let mut end_index = index;
        if index + 1 < numbers.len() {
            step = numbers[index + 1] - start;
            let mut cursor = index + 1;
            while cursor < numbers.len() && numbers[cursor] - last == step {
                last = numbers[cursor];
                end_index = cursor;
                cursor += 1;
            }
        }
        if end_index > index && step != 0.0 {
            let step_text = if step.fract() == 0.0 {
                format!("{}", step as i64)
            } else {
                super::runtime::format_float(step)
            };
            parts.push(format!(
                "{}..{}:{}",
                render_number(start),
                render_number(last),
                step_text
            ));
            index = end_index + 1;
        } else {
            parts.push(render_number(start));
            index += 1;
        }
    }
    parts.join(", ")
}

fn render_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        (value as i64).to_string()
    } else {
        super::runtime::format_float(value)
    }
}
