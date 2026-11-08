use crate::frontend::ast::{BinaryOp, Type};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetDesignator {
    Natural,
    Integer,
    Rational,
    Text,
    Boolean,
    Character,
}

impl SetDesignator {
    pub fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "N" => SetDesignator::Natural,
            "Z" => SetDesignator::Integer,
            "Q" => SetDesignator::Rational,
            "S" => SetDesignator::Text,
            "B" => SetDesignator::Boolean,
            "C" => SetDesignator::Character,
            _ => return None,
        })
    }

    pub fn spelling(&self) -> &'static str {
        match self {
            SetDesignator::Natural => "N",
            SetDesignator::Integer => "Z",
            SetDesignator::Rational => "Q",
            SetDesignator::Text => "S",
            SetDesignator::Boolean => "B",
            SetDesignator::Character => "C",
        }
    }

    pub fn element_type(&self) -> SemaType {
        match self {
            SetDesignator::Natural | SetDesignator::Integer => SemaType::Int,
            SetDesignator::Rational => SemaType::Float,
            SetDesignator::Text => SemaType::Str,
            SetDesignator::Boolean => SemaType::Bool,
            SetDesignator::Character => SemaType::Char,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinNamespace {
    Date,
    Json,
    Store,
    Random,
}

impl BuiltinNamespace {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "date" => BuiltinNamespace::Date,
            "json" => BuiltinNamespace::Json,
            "store" => BuiltinNamespace::Store,
            "random" => BuiltinNamespace::Random,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            BuiltinNamespace::Date => "date",
            BuiltinNamespace::Json => "json",
            BuiltinNamespace::Store => "store",
            BuiltinNamespace::Random => "random",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnShape {
    pub name: String,
    pub ty: SemaType,
    pub auto: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableType {
    pub name: Option<String>,
    pub columns: Vec<ColumnShape>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SemaType {
    Void,
    Int,
    Float,
    Str,
    Bool,
    Char,
    Date,
    Json,
    SetAny,
    Set(SetDesignator),
    Map { key: Box<SemaType>, value: Box<SemaType> },
    Table(TableType),
    Row(Vec<ColumnShape>),
    Array(Vec<SemaType>),
    Keys(Box<SemaType>),
    Namespace(BuiltinNamespace),
    RandomChoice,
}

pub fn sema_type(ast: &Type) -> SemaType {
    match ast {
        Type::Int => SemaType::Int,
        Type::Float => SemaType::Float,
        Type::Str => SemaType::Str,
        Type::Boolean => SemaType::Bool,
        Type::Date => SemaType::Date,
        Type::Json => SemaType::Json,
        Type::Set => SemaType::SetAny,
        Type::Table => SemaType::Table(TableType { name: None, columns: Vec::new() }),
        Type::Map { key, value } => SemaType::Map {
            key: Box::new(sema_type(key)),
            value: Box::new(sema_type(value)),
        },
    }
}

pub fn assignable(expected: &SemaType, actual: &SemaType) -> bool {
    match (expected, actual) {
        (SemaType::SetAny, SemaType::SetAny | SemaType::Set(_)) => true,
        (SemaType::Set(_), SemaType::Set(_)) => true,
        (SemaType::Table(_), SemaType::Table(_)) => true,
        (SemaType::Map { key, value }, SemaType::Map { key: k2, value: v2 }) => {
            assignable(key, k2) && assignable(value, v2)
        }
        _ => expected == actual,
    }
}

pub fn describe(ty: &SemaType) -> String {
    match ty {
        SemaType::Void => "void".to_string(),
        SemaType::Int => "int".to_string(),
        SemaType::Float => "float".to_string(),
        SemaType::Str => "str".to_string(),
        SemaType::Bool => "boolean".to_string(),
        SemaType::Char => "char".to_string(),
        SemaType::Date => "date".to_string(),
        SemaType::Json => "json".to_string(),
        SemaType::SetAny => "set".to_string(),
        SemaType::Set(designator) => format!("set of {}", designator.spelling()),
        SemaType::Map { key, value } => {
            format!("map of {} <-> {}", describe(key), describe(value))
        }
        SemaType::Table(_) => "table".to_string(),
        SemaType::Row(_) => "table row".to_string(),
        SemaType::Array(_) => "array".to_string(),
        SemaType::Keys(_) => "map keys".to_string(),
        SemaType::Namespace(namespace) => format!("builtin `{}`", namespace.name()),
        SemaType::RandomChoice => "random.choice".to_string(),
    }
}

fn numeric_pair(left: &SemaType, right: &SemaType) -> bool {
    matches!(left, SemaType::Int | SemaType::Float)
        && matches!(right, SemaType::Int | SemaType::Float)
}

pub fn binary_result(op: BinaryOp, left: &SemaType, right: &SemaType) -> Result<SemaType, String> {
    match op {
        BinaryOp::Add => match (left, right) {
            (SemaType::Int, SemaType::Int) => Ok(SemaType::Int),
            (SemaType::Float, SemaType::Float) => Ok(SemaType::Float),
            (SemaType::Str, other) | (other, SemaType::Str)
                if matches!(other, SemaType::Str | SemaType::Int | SemaType::Float | SemaType::Bool | SemaType::Char) =>
            {
                Ok(SemaType::Str)
            }
            (SemaType::Date, SemaType::Int) => Ok(SemaType::Date),
            _ if numeric_pair(left, right) => Ok(SemaType::Float),
            _ => Err(format!(
                "cannot apply `+` to {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Sub => match (left, right) {
            (SemaType::Int, SemaType::Int) => Ok(SemaType::Int),
            (SemaType::Float, SemaType::Float) => Ok(SemaType::Float),
            (SemaType::Date, SemaType::Date) => Ok(SemaType::Int),
            (SemaType::Date, SemaType::Int) => Ok(SemaType::Date),
            _ if numeric_pair(left, right) => Ok(SemaType::Float),
            _ => Err(format!(
                "cannot apply `-` to {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Mul => match (left, right) {
            (SemaType::Int, SemaType::Int) => Ok(SemaType::Int),
            (SemaType::Float, SemaType::Float) => Ok(SemaType::Float),
            _ if numeric_pair(left, right) => Ok(SemaType::Float),
            _ => Err(format!(
                "cannot apply `*` to {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Div => match (left, right) {
            (SemaType::Float, SemaType::Float) => Ok(SemaType::Float),
            _ if numeric_pair(left, right) => Ok(SemaType::Float),
            _ => Err(format!(
                "cannot apply `/` to {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Mod => match (left, right) {
            (SemaType::Int, SemaType::Int) => Ok(SemaType::Int),
            _ => Err(format!(
                "cannot apply `%` to {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Concat => match (left, right) {
            (SemaType::Int, SemaType::Int) => Ok(SemaType::Int),
            _ => Err(format!(
                "`++` joins the digits of two ints, got {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Eq | BinaryOp::NotEq => {
            if numeric_pair(left, right)
                || (left == right
                    && !matches!(
                        left,
                        SemaType::Void
                            | SemaType::Table(_)
                            | SemaType::Row(_)
                            | SemaType::Keys(_)
                            | SemaType::Namespace(_)
                            | SemaType::RandomChoice
                            | SemaType::Array(_)
                            | SemaType::SetAny
                    ))
            {
                Ok(SemaType::Bool)
            } else {
                Err(format!(
                    "cannot compare {} and {}",
                    describe(left),
                    describe(right)
                ))
            }
        }
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
            match (left, right) {
                (SemaType::Int, SemaType::Int)
                | (SemaType::Float, SemaType::Float)
                | (SemaType::Str, SemaType::Str)
                | (SemaType::Date, SemaType::Date) => Ok(SemaType::Bool),
                _ if numeric_pair(left, right) => Ok(SemaType::Bool),
                _ => Err(format!(
                    "cannot compare {} and {} with an ordering operator",
                    describe(left),
                    describe(right)
                )),
            }
        }
        BinaryOp::And | BinaryOp::Or => match (left, right) {
            (SemaType::Bool, SemaType::Bool) => Ok(SemaType::Bool),
            _ => Err(format!(
                "`AND` and `OR` require booleans, got {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Has => match (left, right) {
            (SemaType::Str, SemaType::Str) => Ok(SemaType::Bool),
            (SemaType::Set(designator), element) if *element == designator.element_type() => {
                Ok(SemaType::Bool)
            }
            (SemaType::Set(designator), element) => Err(format!(
                "`has` on a set of {} requires an element of type {}, got {}",
                designator.spelling(),
                describe(&designator.element_type()),
                describe(element)
            )),
            (SemaType::Map { key, .. }, probe) if **key == *probe => Ok(SemaType::Bool),
            (SemaType::Map { key, .. }, probe) => Err(format!(
                "`has` on a map requires a key of type {}, got {}",
                describe(key),
                describe(probe)
            )),
            _ => Err(format!(
                "`has` works on str, set and map, got {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::Union
        | BinaryOp::Intersection
        | BinaryOp::Difference
        | BinaryOp::SymmetricDifference => match (left, right) {
            (SemaType::Set(a), SemaType::Set(b)) if a == b => Ok(SemaType::Set(*a)),
            (SemaType::Set(a), SemaType::Set(b)) => Err(format!(
                "set operations require two sets of the same type, got {} and {}",
                a.spelling(),
                b.spelling()
            )),
            _ => Err(format!(
                "set operations work on sets, got {} and {}",
                describe(left),
                describe(right)
            )),
        },
        BinaryOp::From => Err("`from` is only valid after `random.choice`".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn division_of_two_ints_yields_a_float() {
        assert_eq!(
            binary_result(BinaryOp::Div, &SemaType::Int, &SemaType::Int),
            Ok(SemaType::Float)
        );
    }

    #[test]
    fn mixed_int_and_float_arithmetic_yields_a_float() {
        assert_eq!(
            binary_result(BinaryOp::Add, &SemaType::Int, &SemaType::Float),
            Ok(SemaType::Float)
        );
        assert_eq!(
            binary_result(BinaryOp::Sub, &SemaType::Float, &SemaType::Int),
            Ok(SemaType::Float)
        );
        assert_eq!(
            binary_result(BinaryOp::Mul, &SemaType::Float, &SemaType::Int),
            Ok(SemaType::Float)
        );
        assert_eq!(
            binary_result(BinaryOp::Add, &SemaType::Int, &SemaType::Int),
            Ok(SemaType::Int)
        );
    }

    #[test]
    fn string_concatenation_accepts_every_printable_type() {
        for right in [SemaType::Int, SemaType::Float, SemaType::Bool, SemaType::Char, SemaType::Str] {
            assert_eq!(
                binary_result(BinaryOp::Add, &SemaType::Str, &right),
                Ok(SemaType::Str)
            );
        }
    }

    #[test]
    fn set_operations_require_the_same_designator() {
        let natural = SemaType::Set(SetDesignator::Natural);
        let text = SemaType::Set(SetDesignator::Text);
        assert_eq!(
            binary_result(BinaryOp::Union, &natural, &natural),
            Ok(natural.clone())
        );
        assert!(binary_result(BinaryOp::Union, &natural, &text).is_err());
    }

    #[test]
    fn has_is_typed_for_str_set_and_map() {
        assert_eq!(
            binary_result(BinaryOp::Has, &SemaType::Str, &SemaType::Str),
            Ok(SemaType::Bool)
        );
        let integers = SemaType::Set(SetDesignator::Integer);
        assert_eq!(
            binary_result(BinaryOp::Has, &integers, &SemaType::Int),
            Ok(SemaType::Bool)
        );
        let people = SemaType::Map { key: Box::new(SemaType::Str), value: Box::new(SemaType::Int) };
        assert_eq!(
            binary_result(BinaryOp::Has, &people, &SemaType::Float),
            Err("`has` on a map requires a key of type str, got float".to_string())
        );
    }

    #[test]
    fn date_arithmetic_yields_dates_and_day_differences() {
        assert_eq!(
            binary_result(BinaryOp::Add, &SemaType::Date, &SemaType::Int),
            Ok(SemaType::Date)
        );
        assert_eq!(
            binary_result(BinaryOp::Sub, &SemaType::Date, &SemaType::Date),
            Ok(SemaType::Int)
        );
    }
}
