use std::collections::HashMap;

use crate::sema::types::{SemaType, TableType};

#[derive(Clone, Debug, PartialEq)]
pub enum MapLiteralKey {
    Text(String),
    Integer(i64),
}

#[derive(Clone, Debug)]
pub struct VarEntry {
    pub ty: SemaType,
    pub mutable: bool,
    pub map_literal_keys: Option<Vec<MapLiteralKey>>,
    pub json_literal: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct FunctionSignature {
    pub parameters: Vec<SemaType>,
    pub return_type: Option<SemaType>,
}

pub const RESERVED_NAMES: [&str; 4] = ["date", "json", "store", "random"];

#[derive(Default)]
pub struct Environment {
    scopes: Vec<HashMap<String, VarEntry>>,
    values: HashMap<String, VarEntry>,
    tables: HashMap<String, TableType>,
    functions: HashMap<String, FunctionSignature>,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn isolated(&self) -> Self {
        Self {
            scopes: vec![HashMap::new()],
            values: self.values.clone(),
            tables: self.tables.clone(),
            functions: self.functions.clone(),
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare(&mut self, name: &str, entry: VarEntry) -> Result<(), String> {
        if RESERVED_NAMES.contains(&name) {
            return Err(format!(
                "reserved builtin name `{name}` cannot be used as a variable"
            ));
        }
        if self.tables.contains_key(name) {
            return Err(format!("table `{name}` is already declared"));
        }
        if self.values.contains_key(name) {
            return Err(format!("value `{name}` is already declared"));
        }
        let scope = self
            .scopes
            .last_mut()
            .expect("the global scope is always present");
        if scope.contains_key(name) {
            return Err(format!("variable `{name}` is already declared in this scope"));
        }
        scope.insert(name.to_string(), entry);
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&VarEntry> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
    }

    pub fn declare_value(&mut self, name: &str, entry: VarEntry) -> Result<(), String> {
        if RESERVED_NAMES.contains(&name) {
            return Err(format!(
                "reserved builtin name `{name}` cannot be used as a variable"
            ));
        }
        if self.values.contains_key(name) {
            return Err(format!("value `{name}` is already declared"));
        }
        if self.tables.contains_key(name) {
            return Err(format!("table `{name}` is already declared"));
        }
        if self.lookup(name).is_some() {
            return Err(format!("variable `{name}` is already declared"));
        }
        self.values.insert(name.to_string(), entry);
        Ok(())
    }

    pub fn value(&self, name: &str) -> Option<&VarEntry> {
        self.values.get(name)
    }

    pub fn declare_table(&mut self, name: &str, table: TableType) -> Result<(), String> {
        if RESERVED_NAMES.contains(&name) {
            return Err(format!(
                "reserved builtin name `{name}` cannot be used as a table"
            ));
        }
        if self.tables.contains_key(name) {
            return Err(format!("table `{name}` is already declared"));
        }
        if self.lookup(name).is_some() {
            return Err(format!("variable `{name}` is already declared"));
        }
        if self.values.contains_key(name) {
            return Err(format!("value `{name}` is already declared"));
        }
        self.tables.insert(name.to_string(), table);
        Ok(())
    }

    pub fn table(&self, name: &str) -> Option<&TableType> {
        self.tables.get(name)
    }

    pub fn declare_function(&mut self, name: &str, signature: FunctionSignature) -> Result<(), String> {
        if RESERVED_NAMES.contains(&name) {
            return Err(format!(
                "reserved builtin name `{name}` cannot be used as a function name"
            ));
        }
        if self.functions.contains_key(name) {
            return Err(format!("function `{name}` is already declared"));
        }
        self.functions.insert(name.to_string(), signature);
        Ok(())
    }

    pub fn function(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ty: SemaType) -> VarEntry {
        VarEntry { ty, mutable: true, map_literal_keys: None, json_literal: None }
    }

    #[test]
    fn inner_scopes_shadow_outer_declarations() {
        let mut environment = Environment::new();
        environment.push_scope();
        environment
            .declare("x", entry(SemaType::Int))
            .expect("first declaration");
        environment.push_scope();
        environment
            .declare("x", entry(SemaType::Str))
            .expect("shadowing in an inner scope");
        assert_eq!(environment.lookup("x").expect("declared").ty, SemaType::Str);
        environment.pop_scope();
        assert_eq!(environment.lookup("x").expect("declared").ty, SemaType::Int);
    }

    #[test]
    fn duplicate_declaration_in_the_same_scope_is_rejected() {
        let mut environment = Environment::new();
        environment.push_scope();
        environment
            .declare("x", entry(SemaType::Int))
            .expect("first declaration");
        assert!(environment.declare("x", entry(SemaType::Float)).is_err());
    }

    #[test]
    fn builtin_namespaces_cannot_be_shadowed() {
        let mut environment = Environment::new();
        environment.push_scope();
        assert!(environment.declare("date", entry(SemaType::Str)).is_err());
        assert!(environment.declare_function("date", FunctionSignature { parameters: Vec::new(), return_type: None }).is_err());
    }

    fn table(name: &str) -> TableType {
        TableType { name: Some(name.to_string()), columns: Vec::new() }
    }

    #[test]
    fn declared_tables_are_visible_from_isolated_environments() {
        let mut environment = Environment::new();
        environment
            .declare_table("books", table("books"))
            .expect("first table declaration");
        let isolated = environment.isolated();
        assert!(isolated.table("books").is_some());
        assert!(isolated.lookup("books").is_none());
    }

    #[test]
    fn a_table_name_cannot_be_redeclared() {
        let mut environment = Environment::new();
        environment
            .declare_table("books", table("books"))
            .expect("first table declaration");
        assert!(environment.declare_table("books", table("books")).is_err());
        assert!(environment
            .declare("books", entry(SemaType::Int))
            .is_err());
    }
}
