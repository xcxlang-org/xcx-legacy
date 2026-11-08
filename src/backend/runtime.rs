use std::collections::HashMap;

use cranelift::codegen::ir::{types, FuncRef};
use cranelift::frontend::FunctionBuilder;
use cranelift::codegen::ir::AbiParam;
use cranelift_module::{Linkage, Module};
use std::io::Write;

#[unsafe(no_mangle)]
pub extern "C" fn xcx_runtime_finish() {
    super::tables::reset_runtime();
    super::memory::finish();
}

pub fn write_line(text: &str) {
    let mut stdout = std::io::stdout().lock();
    let result = writeln!(stdout, "{text}").and_then(|_| stdout.flush());
    if let Err(error) = result {
        eprintln!("HALT: cannot write output: {error}");
        std::process::exit(1);
    }
}

pub fn halt(message: &str) -> ! {
    let _ = std::io::stdout().flush();
    eprintln!("HALT: {message}");
    std::process::exit(1);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_flush_output() -> i32 {
    match std::io::stdout().flush() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("HALT: cannot flush output: {error}");
            1
        }
    }
}

pub fn format_float(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_int(value: i64) {
    write_line(&value.to_string());
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_float(value: f64) {
    write_line(&format_float(value));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_bool(value: i8) {
    write_line(if value != 0 { "true" } else { "false" });
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_str(ptr: i64, len: i64) {
    write_line(&read_text(ptr, len));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_char(value: i32) {
    match char::from_u32(value as u32) {
        Some(character) => write_line(&character.to_string()),
        None => halt(&format!("invalid character code {value}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_str_contains(
    haystack: i64,
    haystack_len: i64,
    needle: i64,
    needle_len: i64,
) -> i8 {
    let haystack = read_bytes(haystack, haystack_len);
    let needle = read_bytes(needle, needle_len);
    i8::from(is_subsequence(&haystack, &needle))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_str_length(_ptr: i64, len: i64) -> i64 {
    len
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_str_compare(
    left_ptr: i64,
    left_len: i64,
    right_ptr: i64,
    right_len: i64,
) -> i64 {
    let left = read_bytes(left_ptr, left_len);
    let right = read_bytes(right_ptr, right_len);
    match left.cmp(&right) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn is_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

pub fn read_bytes(ptr: i64, len: i64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    if ptr == 0 || len < 0 || len as u64 > isize::MAX as u64 {
        halt("invalid string buffer");
    }
    unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) }.to_vec()
}

pub fn read_text(ptr: i64, len: i64) -> String {
    String::from_utf8_lossy(&read_bytes(ptr, len)).into_owned()
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_ss(
    left: i64,
    left_len: i64,
    right: i64,
    right_len: i64,
    out_len: *mut i64,
) -> i64 {
    let left = read_bytes(left, left_len);
    let right = read_bytes(right, right_len);
    concat_into(&left, &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_si(left: i64, left_len: i64, right: i64, out_len: *mut i64) -> i64 {
    let left = read_bytes(left, left_len);
    let right = right.to_string().into_bytes();
    concat_into(&left, &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_is(left: i64, right: i64, right_len: i64, out_len: *mut i64) -> i64 {
    let left = left.to_string().into_bytes();
    let right = read_bytes(right, right_len);
    concat_into(&left, &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_sf(left: i64, left_len: i64, right: f64, out_len: *mut i64) -> i64 {
    let left = read_bytes(left, left_len);
    let right = format_float(right).into_bytes();
    concat_into(&left, &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_fs(left: f64, right: i64, right_len: i64, out_len: *mut i64) -> i64 {
    let left = format_float(left).into_bytes();
    let right = read_bytes(right, right_len);
    concat_into(&left, &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_sb(left: i64, left_len: i64, right: i8, out_len: *mut i64) -> i64 {
    let left = read_bytes(left, left_len);
    let right = if right != 0 { "true" } else { "false" };
    concat_into(&left, right.as_bytes(), out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_bs(left: i8, right: i64, right_len: i64, out_len: *mut i64) -> i64 {
    let left = if left != 0 { "true" } else { "false" };
    let right = read_bytes(right, right_len);
    concat_into(left.as_bytes(), &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_sc(left: i64, left_len: i64, right: i32, out_len: *mut i64) -> i64 {
    let left = read_bytes(left, left_len);
    let right = char_from_code(right).to_string().into_bytes();
    concat_into(&left, &right, out_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_cs(left: i32, right: i64, right_len: i64, out_len: *mut i64) -> i64 {
    let left = char_from_code(left).to_string().into_bytes();
    let right = read_bytes(right, right_len);
    concat_into(&left, &right, out_len)
}

fn char_from_code(code: i32) -> char {
    char::from_u32(code as u32).unwrap_or('\u{FFFD}')
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_concat_ii(left: i64, right: i64) -> i64 {
    let joined = format!("{left}{right}");
    match joined.parse::<i64>() {
        Ok(value) => value,
        Err(_) => halt(&format!("integer concatenation `{left} ++ {right}` overflows i64")),
    }
}

fn concat_into(left: &[u8], right: &[u8], out_len: *mut i64) -> i64 {
    let mut bytes = Vec::with_capacity(left.len() + right.len());
    bytes.extend_from_slice(left);
    bytes.extend_from_slice(right);
    unsafe { *out_len = bytes.len() as i64 };
    super::memory::allocate_bytes(bytes)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_halt() {
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

type Parameters = &'static [types::Type];
type SignatureEntry = (&'static str, Parameters, Option<types::Type>);

const I: types::Type = types::I64;
const F: types::Type = types::F64;
const B: types::Type = types::I8;
const C: types::Type = types::I32;

pub const SIGNATURES: &[SignatureEntry] = &[
    ("xcx_frame_enter", &[], None),
    ("xcx_frame_leave", &[], None),
    ("xcx_root_set", &[I, I], None),
    ("xcx_collect", &[], None),
    ("xcx_print_int", &[I], None),
    ("xcx_print_float", &[F], None),
    ("xcx_print_bool", &[B], None),
    ("xcx_print_str", &[I, I], None),
    ("xcx_print_char", &[C], None),
    ("xcx_print_set", &[I], None),
    ("xcx_str_contains", &[I, I, I, I], Some(B)),
    ("xcx_str_length", &[I, I], Some(I)),
    ("xcx_str_compare", &[I, I, I, I], Some(I)),
    ("xcx_concat_ss", &[I, I, I, I, I], Some(I)),
    ("xcx_concat_si", &[I, I, I, I], Some(I)),
    ("xcx_concat_is", &[I, I, I, I], Some(I)),
    ("xcx_concat_sf", &[I, I, F, I], Some(I)),
    ("xcx_concat_fs", &[F, I, I, I], Some(I)),
    ("xcx_concat_sb", &[I, I, B, I], Some(I)),
    ("xcx_concat_bs", &[B, I, I, I], Some(I)),
    ("xcx_concat_sc", &[I, I, C, I], Some(I)),
    ("xcx_concat_cs", &[C, I, I, I], Some(I)),
    ("xcx_concat_ii", &[I, I], Some(I)),
    ("xcx_halt", &[], None),
    ("xcx_set_builder_new", &[C], Some(I)),
    ("xcx_set_add_int", &[I, I], None),
    ("xcx_set_add_float", &[I, F], None),
    ("xcx_set_add_str", &[I, I, I], None),
    ("xcx_set_add_bool", &[I, B], None),
    ("xcx_set_add_char", &[I, C], None),
    ("xcx_set_add_range_int", &[I, I, I, I], None),
    ("xcx_set_add_range_float", &[I, F, F, F], None),
    ("xcx_set_builder_finish", &[I], Some(I)),
    ("xcx_set_union", &[I, I], Some(I)),
    ("xcx_set_intersection", &[I, I], Some(I)),
    ("xcx_set_difference", &[I, I], Some(I)),
    ("xcx_set_symmetric_difference", &[I, I], Some(I)),
    ("xcx_set_has_int", &[I, I], Some(B)),
    ("xcx_set_has_float", &[I, F], Some(B)),
    ("xcx_set_has_str", &[I, I, I], Some(B)),
    ("xcx_set_has_bool", &[I, B], Some(B)),
    ("xcx_set_has_char", &[I, C], Some(B)),
    ("xcx_set_is_empty", &[I], Some(B)),
    ("xcx_choice_int", &[I], Some(I)),
    ("xcx_choice_float", &[I], Some(F)),
    ("xcx_choice_bool", &[I], Some(B)),
    ("xcx_choice_char", &[I], Some(C)),
    ("xcx_choice_str", &[I, I], Some(I)),
    ("xcx_map_new", &[], Some(I)),
    ("xcx_map_ii_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_if_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_ib_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_ic_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_is_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_si_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_sf_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_sb_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_sc_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_ss_insert", &[I, I, I, I, I, F], None),
    ("xcx_map_ii_get", &[I, I, I, I], Some(I)),
    ("xcx_map_if_get", &[I, I, I, I], Some(F)),
    ("xcx_map_ib_get", &[I, I, I, I], Some(B)),
    ("xcx_map_ic_get", &[I, I, I, I], Some(C)),
    ("xcx_map_is_get", &[I, I, I, I], Some(I)),
    ("xcx_map_si_get", &[I, I, I, I], Some(I)),
    ("xcx_map_sf_get", &[I, I, I, I], Some(F)),
    ("xcx_map_sb_get", &[I, I, I, I], Some(B)),
    ("xcx_map_sc_get", &[I, I, I, I], Some(C)),
    ("xcx_map_ss_get", &[I, I, I, I], Some(I)),
    ("xcx_map_i_has", &[I, I], Some(B)),
    ("xcx_map_s_has", &[I, I, I], Some(B)),
    ("xcx_map_i_keys", &[I], Some(I)),
    ("xcx_map_s_keys", &[I], Some(I)),
    ("xcx_map_size", &[I], Some(I)),
    ("xcx_map_show", &[I], None),
    ("xcx_keys_len", &[I], Some(I)),
    ("xcx_keys_get_int", &[I, I], Some(I)),
    ("xcx_keys_get_str", &[I, I, I], Some(I)),
    ("xcx_table_new_empty", &[I], Some(I)),
    ("xcx_table_shape_decode", &[I, I], Some(I)),
    ("xcx_table_insert", &[I, I, I], None),
    ("xcx_table_update", &[I, I, I, I], None),
    ("xcx_table_delete", &[I, I], None),
    ("xcx_table_count", &[I], Some(I)),
    ("xcx_table_fill_context", &[I, I, I], None),
    ("xcx_table_append_row", &[I, I, I], None),
    ("xcx_table_get_int", &[I, I, I], Some(I)),
    ("xcx_table_get_float", &[I, I, I], Some(F)),
    ("xcx_table_get_str", &[I, I, I, I], Some(I)),
    ("xcx_table_get_bool", &[I, I, I], Some(B)),
    ("xcx_table_get_char", &[I, I, I], Some(C)),
    ("xcx_table_get_date", &[I, I, I], Some(I)),
    ("xcx_table_print", &[I], None),
    ("xcx_table_join", &[I, I, I, I, I, I, I, I], Some(I)),
    ("xcx_global_table", &[I, I], Some(I)),
    ("xcx_global_table_register", &[I, I, I], None),
    ("xcx_global_map", &[I, I], Some(I)),
    ("xcx_global_map_register", &[I, I, I], None),
    ("xcx_date_parse", &[I, I], Some(I)),
    ("xcx_date_parse_format", &[I, I, I, I], Some(I)),
    ("xcx_date_now", &[], Some(I)),
    ("xcx_date_year", &[I], Some(I)),
    ("xcx_date_month", &[I], Some(I)),
    ("xcx_date_day", &[I], Some(I)),
    ("xcx_date_format", &[I, I, I, I], Some(I)),
    ("xcx_print_date", &[I], None),
    ("xcx_json_parse", &[I, I], Some(I)),
    ("xcx_json_count", &[I], Some(I)),
    ("xcx_json_field", &[I, I, I], Some(I)),
    ("xcx_json_exists", &[I, I, I], Some(B)),
    ("xcx_json_get_int", &[I, I, I], Some(I)),
    ("xcx_json_get_float", &[I, I, I], Some(F)),
    ("xcx_json_get_str", &[I, I, I, I], Some(I)),
    ("xcx_json_get_bool", &[I, I, I], Some(B)),
    ("xcx_json_to_str", &[I, I], Some(I)),
    ("xcx_json_print", &[I], None),
    ("xcx_json_inject", &[I, I, I], None),
    ("xcx_store_write", &[I, I, I, I], None),
    ("xcx_store_read", &[I, I, I], Some(I)),
    ("xcx_store_append", &[I, I, I, I], None),
    ("xcx_store_exists", &[I, I], Some(B)),
    ("xcx_read_str", &[I], Some(I)),
    ("xcx_read_int", &[], Some(I)),
    ("xcx_read_float", &[], Some(F)),
    ("xcx_read_bool", &[], Some(B)),
    ("xcx_terminal_clean", &[], None),
    ("xcx_terminal_help", &[], None),
];

#[derive(Clone)]
pub struct Helpers {
    references: HashMap<String, FuncRef>,
}

impl Helpers {
    pub fn declare(module: &mut dyn Module, builder: &mut FunctionBuilder) -> Self {
        let mut references = HashMap::new();
        for (name, parameters, returns) in SIGNATURES {
            let mut signature = module.make_signature();
            signature
                .params
                .extend(parameters.iter().map(|&ty| AbiParam::new(ty)));
            if let Some(return_type) = returns {
                signature.returns.push(AbiParam::new(*return_type));
            }
            let id = module
                .declare_function(name, Linkage::Import, &signature)
                .expect("declaring a runtime helper");
            let reference = module.declare_func_in_func(id, &mut builder.func);
            references.insert((*name).to_string(), reference);
        }
        Self { references }
    }

    pub fn reference(&self, name: &str) -> FuncRef {
        self.references
            .get(name)
            .copied()
            .unwrap_or_else(|| panic!("runtime helper `{name}` is not declared"))
    }
}
