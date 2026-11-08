use crate::frontend::ast::Type;
use cranelift::codegen::ir::types;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Set,
    Map,
    Table,
}

#[derive(Clone, Copy)]
pub struct GlobalOpaque {
    pub kind: ObjectKind,
    pub map_value: Option<char>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Scalar(types::Type),
    StrPair,
    Opaque(ObjectKind),
    Days,
    JsonBuffer,
}

pub fn storage(ast: &Type) -> Option<Storage> {
    match ast {
        Type::Int => Some(Storage::Scalar(types::I64)),
        Type::Float => Some(Storage::Scalar(types::F64)),
        Type::Boolean => Some(Storage::Scalar(types::I8)),
        Type::Str => Some(Storage::StrPair),
        Type::Set => Some(Storage::Opaque(ObjectKind::Set)),
        Type::Map { .. } => Some(Storage::Opaque(ObjectKind::Map)),
        Type::Table => Some(Storage::Opaque(ObjectKind::Table)),
        Type::Date => Some(Storage::Days),
        Type::Json => Some(Storage::JsonBuffer),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReturnShape {
    Void,
    Scalar(types::Type),
    Str,
    Opaque,
    Days,
}

pub fn return_shape(return_type: Option<&Type>) -> Option<ReturnShape> {
    match return_type {
        None => Some(ReturnShape::Void),
        Some(Type::Int) => Some(ReturnShape::Scalar(types::I64)),
        Some(Type::Float) => Some(ReturnShape::Scalar(types::F64)),
        Some(Type::Boolean) => Some(ReturnShape::Scalar(types::I8)),
        Some(Type::Str) => Some(ReturnShape::Str),
        Some(Type::Table) => Some(ReturnShape::Opaque),
        Some(Type::Date) => Some(ReturnShape::Days),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotLayout {
    Scalar(types::Type),
    Str,
    Opaque,
    Days,
}

pub fn slot_layout(ast: &Type) -> Option<SlotLayout> {
    match ast {
        Type::Int => Some(SlotLayout::Scalar(types::I64)),
        Type::Float => Some(SlotLayout::Scalar(types::F64)),
        Type::Boolean => Some(SlotLayout::Scalar(types::I8)),
        Type::Str => Some(SlotLayout::Str),
        Type::Table => Some(SlotLayout::Opaque),
        Type::Date => Some(SlotLayout::Days),
        _ => None,
    }
}

pub fn flatten_slots(slots: &[SlotLayout]) -> Vec<types::Type> {
    slots
        .iter()
        .flat_map(|slot| match slot {
            SlotLayout::Scalar(ty) => vec![*ty],
            SlotLayout::Str => vec![types::I64, types::I64],
            SlotLayout::Opaque | SlotLayout::Days => vec![types::I64],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_types_map_to_cranelift_types() {
        assert_eq!(storage(&Type::Int), Some(Storage::Scalar(types::I64)));
        assert_eq!(storage(&Type::Float), Some(Storage::Scalar(types::F64)));
        assert_eq!(storage(&Type::Boolean), Some(Storage::Scalar(types::I8)));
        assert_eq!(storage(&Type::Str), Some(Storage::StrPair));
    }

    #[test]
    fn sets_and_maps_are_opaque_pointers() {
        assert_eq!(
            storage(&Type::Set),
            Some(Storage::Opaque(ObjectKind::Set))
        );
        assert_eq!(
            storage(&Type::Map { key: Box::new(Type::Str), value: Box::new(Type::Int) }),
            Some(Storage::Opaque(ObjectKind::Map))
        );
        assert_eq!(storage(&Type::Date), Some(Storage::Days));
    }

    #[test]
    fn str_parameters_take_two_machine_slots() {
        assert_eq!(
            flatten_slots(&[SlotLayout::Scalar(types::I64), SlotLayout::Str]),
            vec![types::I64, types::I64, types::I64]
        );
    }

    #[test]
    fn return_shapes_follow_the_documented_types() {
        assert_eq!(return_shape(None), Some(ReturnShape::Void));
        assert_eq!(return_shape(Some(&Type::Str)), Some(ReturnShape::Str));
        assert_eq!(
            return_shape(Some(&Type::Table)),
            Some(ReturnShape::Opaque)
        );
    }
}
