use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use super::runtime;
use crate::frontend::ast::Type;

pub const KIND_INT: i32 = 0;
pub const KIND_FLOAT: i32 = 1;
pub const KIND_TEXT: i32 = 2;
pub const KIND_BOOL: i32 = 3;
pub const KIND_CHAR: i32 = 4;
pub const KIND_DATE: i32 = 5;

pub fn column_kind(ast: &Type) -> Option<i32> {
    match ast {
        Type::Int => Some(KIND_INT),
        Type::Float => Some(KIND_FLOAT),
        Type::Str => Some(KIND_TEXT),
        Type::Boolean => Some(KIND_BOOL),
        Type::Date => Some(KIND_DATE),
        _ => None,
    }
}

pub fn column_kind_from_sema(ast: &crate::sema::types::SemaType) -> Option<i32> {
    use crate::sema::types::SemaType;
    match ast {
        SemaType::Int => Some(KIND_INT),
        SemaType::Float => Some(KIND_FLOAT),
        SemaType::Str => Some(KIND_TEXT),
        SemaType::Bool => Some(KIND_BOOL),
        SemaType::Date => Some(KIND_DATE),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TableCell {
    Int(i64),
    Float(f64),
    Text(i64, i64),
    Bool(bool),
    Char(char),
    Date(i64),
}

impl PartialEq for TableCell {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TableCell::Int(a), TableCell::Int(b)) => a == b,
            (TableCell::Float(a), TableCell::Float(b)) => a == b,
            (TableCell::Bool(a), TableCell::Bool(b)) => a == b,
            (TableCell::Char(a), TableCell::Char(b)) => a == b,
            (TableCell::Date(a), TableCell::Date(b)) => a == b,
            (TableCell::Text(ap, al), TableCell::Text(bp, bl)) => {
                al == bl
                    && unsafe { std::slice::from_raw_parts(*ap as *const u8, *al as usize) }
                        == unsafe { std::slice::from_raw_parts(*bp as *const u8, *bl as usize) }
            }
            _ => false,
        }
    }
}

impl TableCell {
    fn matches_kind(&self, kind: i32) -> bool {
        matches!(
            (self, kind),
            (TableCell::Int(_), KIND_INT)
                | (TableCell::Float(_), KIND_FLOAT)
                | (TableCell::Text(_, _), KIND_TEXT)
                | (TableCell::Bool(_), KIND_BOOL)
                | (TableCell::Char(_), KIND_CHAR)
                | (TableCell::Date(_), KIND_DATE)
        )
    }

    fn display(&self) -> String {
        match self {
            TableCell::Int(value) => value.to_string(),
            TableCell::Float(value) => runtime::format_float(*value),
            TableCell::Text(ptr, len) => runtime::read_text(*ptr, *len),
            TableCell::Bool(value) => value.to_string(),
            TableCell::Char(value) => value.to_string(),
            TableCell::Date(days) => {
                let (year, month, day) = super::dates::civil_from_days(*days);
                crate::sema::dates::format_date(year, month, day, crate::sema::dates::DEFAULT_FORMAT)
            }
        }
    }
}

#[derive(Debug)]
pub struct TableShape {
    pub name: String,
    pub columns: Vec<(String, i32, bool)>,
}

pub struct TableObject {
    shape: *mut TableShape,
    rows: Vec<Vec<TableCell>>,
    next_id: i64,
}

impl super::memory::Managed for TableShape {}

impl super::memory::Managed for TableObject {
    fn trace(&self, visit: &mut dyn FnMut(i64)) {
        visit(self.shape as i64);
        for row in &self.rows {
            for cell in row {
                if let TableCell::Text(pointer, _) = cell {
                    visit(*pointer);
                }
            }
        }
    }
}

pub(crate) fn memory_roots() -> Vec<i64> {
    let mut roots = GLOBAL_VALUES.with(|values| values.borrow().values().copied().collect::<Vec<_>>());
    DECODED_SHAPES.with(|shapes| roots.extend(shapes.borrow().values().copied()));
    roots
}

pub(crate) fn reset_runtime() {
    GLOBAL_VALUES.with(|values| values.borrow_mut().clear());
    DECODED_SHAPES.with(|shapes| shapes.borrow_mut().clear());
}

thread_local! {
    static GLOBAL_VALUES: RefCell<HashMap<String, i64>> = RefCell::new(HashMap::new());
}

fn global_value(name: &str) -> i64 {
    match GLOBAL_VALUES.with(|values| values.borrow().get(name).copied()) {
        Some(value) => value,
        None => runtime::halt(&format!("undefined `{name}`")),
    }
}

fn register_global_value(name: String, value: i64) {
    GLOBAL_VALUES.with(|values| {
        values.borrow_mut().insert(name, value);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_global_table(ptr: i64, len: i64) -> i64 {
    global_value(&runtime::read_text(ptr, len))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_global_table_register(ptr: i64, len: i64, table: i64) {
    register_global_value(runtime::read_text(ptr, len), table);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_global_map(ptr: i64, len: i64) -> i64 {
    global_value(&runtime::read_text(ptr, len))
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_global_map_register(ptr: i64, len: i64, map: i64) {
    register_global_value(runtime::read_text(ptr, len), map);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_new_empty(shape: i64) -> i64 {
    super::memory::allocate(TableObject {
        shape: shape as *mut TableShape,
        rows: Vec::new(),
        next_id: 1,
    })
}

fn as_table(ptr: i64) -> &'static TableObject {
    unsafe { &*(ptr as *const TableObject) }
}

fn as_table_mut(ptr: i64) -> &'static mut TableObject {
    unsafe { &mut *(ptr as *mut TableObject) }
}

fn shape_of(table: i64) -> &'static TableShape {
    unsafe { &*(as_table(table).shape) }
}

fn decode_cell(kind: i32, tag: i64, a: i64, b: i64) -> TableCell {
    let cell = match tag {
        0 => TableCell::Int(a),
        1 => TableCell::Float(f64::from_bits(a as u64)),
        2 => TableCell::Text(a, b),
        3 => TableCell::Bool(a != 0),
        4 => TableCell::Char(char::from_u32(a as u32).unwrap_or('\u{FFFD}')),
        5 => TableCell::Date(a),
        _ => runtime::halt("invalid cell tag in a table operation"),
    };
    if !cell.matches_kind(kind) {
        runtime::halt("table cell type does not match the column type");
    }
    cell
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_insert(table: i64, cells: i64, count: i64) {
    let shape = shape_of(table);
    let auto_columns = shape.columns.iter().filter(|(_, _, auto)| *auto).count();
    let expected = shape.columns.len() - auto_columns;
    if count != expected as i64 {
        runtime::halt(&format!(
            "`insert` expects {expected} values (one per non-auto column), got {count}"
        ));
    }
    let raw = unsafe { std::slice::from_raw_parts(cells as *const [i64; 3], count as usize) };
    let mut row: Vec<TableCell> = Vec::with_capacity(shape.columns.len());
    let mut cell_index = 0usize;
    for (column_name, kind, auto) in &shape.columns {
        if *auto {
            row.push(TableCell::Int(0));
            continue;
        }
        let _ = column_name;
        let [tag, a, b] = raw[cell_index];
        row.push(decode_cell(*kind, tag, a, b));
        cell_index += 1;
    }
    append_prepared_row(table, row);
}

pub(crate) fn append_prepared_row(table: i64, cells: Vec<TableCell>) {
    let shape = shape_of(table);
    let object = as_table_mut(table);
    if cells.len() != shape.columns.len() {
        runtime::halt("internal: a prepared row does not match the table shape");
    }
    let mut row = cells;
    for (position, (column_name, _, auto)) in shape.columns.iter().enumerate() {
        if !*auto {
            continue;
        }
        if column_name == "id" {
            row[position] = TableCell::Int(object.next_id);
            object.next_id += 1;
        } else {
            runtime::halt(&format!(
                "auto column `{column_name}` is only supported as the `id` column"
            ));
        }
    }
    object.rows.push(row);
}

pub(crate) fn shape_of_public(table: i64) -> &'static TableShape {
    shape_of(table)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_update(table: i64, row: i64, cells: i64, count: i64) {
    let shape = shape_of(table);
    let object = as_table_mut(table);
    let index = match usize::try_from(row) {
        Ok(index) if (index as i64) < object.rows.len() as i64 => index,
        _ => runtime::halt(&format!("table row {row} does not exist")),
    };
    let auto_columns = shape.columns.iter().filter(|(_, _, auto)| *auto).count();
    let expected = shape.columns.len() - auto_columns;
    if count != expected as i64 {
        runtime::halt(&format!(
            "`update` expects {expected} values (one per non-auto column), got {count}"
        ));
    }
    let raw = unsafe { std::slice::from_raw_parts(cells as *const [i64; 3], count as usize) };
    let mut updated: Vec<TableCell> = object.rows[index].clone();
    let mut cell_index = 0usize;
    for (position, (name, kind, auto)) in shape.columns.iter().enumerate() {
        if *auto {
            continue;
        }
        let [tag, a, b] = raw[cell_index];
        let cell = decode_cell(*kind, tag, a, b);
        if *name == "id" {
            runtime::halt("the `id` column cannot be updated");
        }
        updated[position] = cell;
        cell_index += 1;
    }
    object.rows[index] = updated;
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_delete(table: i64, row: i64) {
    let object = as_table_mut(table);
    let index = match usize::try_from(row) {
        Ok(index) if (index as i64) < object.rows.len() as i64 => index,
        _ => runtime::halt(&format!("table row {row} does not exist")),
    };
    object.rows.remove(index);
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_count(table: i64) -> i64 {
    as_table(table).rows.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_fill_context(table: i64, row: i64, ctx: i64) {
    let object = as_table(table);
    let index = row as usize;
    let slots = unsafe { std::slice::from_raw_parts_mut(ctx as *mut [i64; 2], object.rows[index].len()) };
    for (position, cell) in object.rows[index].iter().enumerate() {
        let (a, b) = match cell {
            TableCell::Int(value) => (*value, 0),
            TableCell::Float(value) => (value.to_bits() as i64, 0),
            TableCell::Text(ptr, len) => (*ptr, *len),
            TableCell::Bool(value) => (i64::from(*value), 0),
            TableCell::Char(value) => (*value as u32 as i64, 0),
            TableCell::Date(value) => (*value, 0),
        };
        slots[position] = [a, b];
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_append_row(dst: i64, src: i64, row: i64) {
    let source = as_table(src);
    let copied = match source.rows.get(row as usize) {
        Some(cells) => cells.clone(),
        None => runtime::halt("table row does not exist"),
    };
    as_table_mut(dst).rows.push(copied);
}

fn cell_at(table: i64, row: i64, column: i64) -> TableCell {
    let object = as_table(table);
    let shape = shape_of(table);
    let Some(cells) = object.rows.get(row as usize) else {
        runtime::halt(&format!("table row {row} does not exist"));
    };
    let column_index = column as usize;
    match cells.get(column_index) {
        Some(cell) => cell.clone(),
        None => runtime::halt(&format!(
            "column `{}` does not exist in row {row}",
            shape
                .columns
                .get(column_index)
                .map(|(name, _, _)| name.clone())
                .unwrap_or_else(|| column.to_string())
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_get_int(table: i64, row: i64, column: i64) -> i64 {
    match cell_at(table, row, column) {
        TableCell::Int(value) => value,
        _ => runtime::halt("table cell is not an int"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_get_float(table: i64, row: i64, column: i64) -> f64 {
    match cell_at(table, row, column) {
        TableCell::Float(value) => value,
        _ => runtime::halt("table cell is not a float"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_get_str(table: i64, row: i64, column: i64, out_len: *mut i64) -> i64 {
    match cell_at(table, row, column) {
        TableCell::Text(ptr, len) => {
            unsafe { *out_len = len };
            ptr
        }
        _ => runtime::halt("table cell is not a str"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_get_bool(table: i64, row: i64, column: i64) -> i8 {
    match cell_at(table, row, column) {
        TableCell::Bool(value) => i8::from(value),
        _ => runtime::halt("table cell is not a boolean"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_get_char(table: i64, row: i64, column: i64) -> i32 {
    match cell_at(table, row, column) {
        TableCell::Char(value) => value as i32,
        _ => runtime::halt("table cell is not a char"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_get_date(table: i64, row: i64, column: i64) -> i64 {
    match cell_at(table, row, column) {
        TableCell::Date(days) => days,
        _ => runtime::halt("table cell is not a date"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_print(table: i64) {
    let object = as_table(table);
    let shape = shape_of(table);
    for row in &object.rows {
        let rendered: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(position, cell)| {
                let name = shape
                    .columns
                    .get(position)
                    .map(|(name, _, _)| name.clone())
                    .unwrap_or_else(|| position.to_string());
                format!("{}: {}", name, cell.display())
            })
            .collect();
        runtime::write_line(&format!("{{ {} }}", rendered.join(", ")));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_join(
    left: i64,
    right: i64,
    left_key: i64,
    right_key: i64,
    left_name_ptr: i64,
    left_name_len: i64,
    right_name_ptr: i64,
    right_name_len: i64,
) -> i64 {
    let left_table = as_table(left);
    let right_table = as_table(right);
    let left_shape = shape_of(left);
    let right_shape = shape_of(right);
    let left_name = runtime::read_text(left_name_ptr, left_name_len);
    let right_name = runtime::read_text(right_name_ptr, right_name_len);

    let layout = join_layout(
        left_shape,
        right_shape,
        left_key as usize,
        right_key as usize,
        &left_name,
        &right_name,
    );

    let shape = super::memory::allocate(TableShape {
        name: format!("{left_name}_{right_name}"),
        columns: layout
            .columns
            .iter()
            .map(|(name, kind)| (name.clone(), *kind, false))
            .collect(),
    });

    let mut rows: Vec<Vec<TableCell>> = Vec::new();
    for left_row in &left_table.rows {
        let left_key_cell = left_row.get(left_key as usize).cloned();
        for right_row in &right_table.rows {
            let right_key_cell = right_row.get(right_key as usize).cloned();
            let matches = match (left_key_cell.clone(), right_key_cell.clone()) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            };
            if !matches {
                continue;
            }
            let mut merged: Vec<TableCell> = Vec::with_capacity(layout.columns.len());
            for (side, index) in &layout.mapping {
                let row = if *side { right_row } else { left_row };
                merged.push(row[*index].clone());
            }
            rows.push(merged);
        }
    }

    super::memory::allocate(TableObject {
        shape: shape as *mut TableShape,
        rows,
        next_id: 1,
    })
}

static WHERE_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn next_where_name() -> String {
    format!("xcx_where_{}", WHERE_COUNTER.fetch_add(1, Ordering::Relaxed))
}

thread_local! {
    static DECODED_SHAPES: RefCell<HashMap<Vec<u8>, i64>> = RefCell::new(HashMap::new());
}

pub fn encode_shape(shape: &TableShape) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(shape.name.len() as u32).to_le_bytes());
    bytes.extend_from_slice(shape.name.as_bytes());
    bytes.extend_from_slice(&(shape.columns.len() as u32).to_le_bytes());
    for (name, kind, auto) in &shape.columns {
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&i32::from(*auto).to_le_bytes());
    }
    bytes
}

fn decode_shape(bytes: &[u8]) -> TableShape {
    let mut cursor = 0usize;
    fn read_u32(bytes: &[u8], cursor: &mut usize) -> u32 {
        let value = u32::from_le_bytes(
            bytes[*cursor..*cursor + 4]
                .try_into()
                .expect("the shape encoding is complete"),
        );
        *cursor += 4;
        value
    }
    fn read_text(bytes: &[u8], cursor: &mut usize) -> String {
        let length = read_u32(bytes, cursor) as usize;
        let text = String::from_utf8_lossy(&bytes[*cursor..*cursor + length]).into_owned();
        *cursor += length;
        text
    }
    let name = read_text(bytes, &mut cursor);
    let column_count = read_u32(bytes, &mut cursor) as usize;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        let column_name = read_text(bytes, &mut cursor);
        let kind = read_u32(bytes, &mut cursor) as i32;
        let auto = read_u32(bytes, &mut cursor) != 0;
        columns.push((column_name, kind, auto));
    }
    TableShape { name, columns }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_table_shape_decode(ptr: i64, len: i64) -> i64 {
    let bytes = runtime::read_bytes(ptr, len);
    let cached = DECODED_SHAPES.with(|shapes| shapes.borrow().get(&bytes).copied());
    if let Some(shape) = cached {
        return shape;
    }
    let shape = super::memory::allocate(decode_shape(&bytes));
    DECODED_SHAPES.with(|shapes| {
        shapes.borrow_mut().insert(bytes, shape);
    });
    shape
}

pub struct JoinLayout {
    pub columns: Vec<(String, i32)>,
    pub mapping: Vec<(bool, usize)>,
}

pub fn join_layout(
    left: &TableShape,
    right: &TableShape,
    left_key: usize,
    right_key: usize,
    left_name: &str,
    right_name: &str,
) -> JoinLayout {
    let mut columns: Vec<(String, i32)> = Vec::new();
    let mut mapping: Vec<(bool, usize)> = Vec::new();

    for (position, (name, kind, _)) in left.columns.iter().enumerate() {
        if position == left_key {
            columns.push((name.clone(), *kind));
            mapping.push((false, position));
            continue;
        }
        if right.columns.iter().any(|(other, _, _)| other == name) {
            columns.push((format!("{left_name}_{name}"), *kind));
            mapping.push((false, position));
            continue;
        }
        columns.push((name.clone(), *kind));
        mapping.push((false, position));
    }
    for (position, (name, kind, _)) in right.columns.iter().enumerate() {
        if position == right_key {
            continue;
        }
        if left.columns.iter().any(|(other, _, _)| other == name) {
            columns.push((format!("{right_name}_{name}"), *kind));
            mapping.push((true, position));
            continue;
        }
        columns.push((name.clone(), *kind));
        mapping.push((true, position));
    }

    JoinLayout { columns, mapping }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shape() -> i64 {
        super::super::memory::allocate(TableShape {
            name: "test".to_string(),
            columns: vec![
                ("id".to_string(), KIND_INT, true),
                ("name".to_string(), KIND_TEXT, false),
            ],
        })
    }

    #[test]
    fn insert_gets_update_and_delete_work_on_rows() {
        let shape = test_shape();
        let table = xcx_table_new_empty(shape);
        let book = b"Book";
        let book_cells = [[KIND_TEXT as i64, book.as_ptr() as i64, 4i64]];
        xcx_table_insert(table, book_cells.as_ptr() as i64, 1);
        let other = b"Other";
        let other_cells = [[KIND_TEXT as i64, other.as_ptr() as i64, 5i64]];
        xcx_table_insert(table, other_cells.as_ptr() as i64, 1);
        assert_eq!(xcx_table_count(table), 2);
        assert_eq!(xcx_table_get_int(table, 0, 0), 1);
        assert_eq!(xcx_table_get_int(table, 1, 0), 2);
        let mut out_len: i64 = 0;
        let ptr = xcx_table_get_str(table, 0, 1, &mut out_len);
        assert_eq!(runtime::read_text(ptr, out_len), "Book");

        let replacement = [[KIND_TEXT as i64, other.as_ptr() as i64, 5]];
        xcx_table_update(table, 0, replacement.as_ptr() as i64, 1);
        let ptr = xcx_table_get_str(table, 0, 1, &mut out_len);
        assert_eq!(runtime::read_text(ptr, out_len), "Other");

        xcx_table_delete(table, 1);
        assert_eq!(xcx_table_count(table), 1);
    }

    #[test]
    fn fill_context_exposes_row_cells() {
        let shape = test_shape();
        let table = xcx_table_new_empty(shape);
        let name = b"N";
        let cells = vec![[KIND_TEXT as i64, name.as_ptr() as i64, 1]];
        xcx_table_insert(table, cells.as_ptr() as i64, 1);
        let mut ctx = vec![[0i64; 2]; 2];
        xcx_table_fill_context(table, 0, ctx.as_mut_ptr() as i64);
        assert_eq!(ctx[0], [1, 0]);
        assert_eq!(ctx[1][1], 1);
    }
}
