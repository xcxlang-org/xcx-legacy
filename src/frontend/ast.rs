use crate::diagnostics::Location;

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub location: Location,
}

pub type Statement = Spanned<Stmt>;
pub type Expression = Spanned<Expr>;
pub type Block = Vec<Statement>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Str,
    Boolean,
    Date,
    Json,
    Set,
    Table,
    Map { key: Box<Type>, value: Box<Type> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Concat,
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Has,
    From,
    Union,
    Intersection,
    Difference,
    SymmetricDifference,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Char(char),
    Bool(bool),
    Ident(String),
    RawJson(String),
    Array(Vec<Expression>),
    MapLiteral(Vec<MapEntry>),
    Unary { op: UnaryOp, operand: Box<Expression> },
    Binary { op: BinaryOp, left: Box<Expression>, right: Box<Expression> },
    FieldAccess { receiver: Box<Expression>, field: String },
    Index { receiver: Box<Expression>, index: Box<Expression> },
    MethodCall { receiver: Box<Expression>, method: String, arguments: Vec<Expression> },
    Call { callee: String, arguments: Vec<Expression> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub key: Expression,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetElement {
    Value(Expression),
    Range { start: Expression, end: Expression, step: Option<Expression> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub ty: Type,
    pub auto: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub name: String,
    pub init: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub condition: Expression,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let { ty: Type, bindings: Vec<Binding> },
    Const { name: String, ty: Type, init: Expression },
    LetSet { name: String, designator: String, elements: Vec<SetElement> },
    LetTable { name: String, columns: Vec<Column>, init: Option<Expression> },
    Assign { name: String, value: Expression },
    Print { value: Expression },
    Read { name: String },
    Halt,
    Break,
    Continue,
    Return { value: Option<Expression> },
    If { branches: Vec<Branch>, otherwise: Option<Block> },
    While { condition: Expression, body: Block },
    ForRange { variable: String, start: Expression, end: Expression, body: Block },
    ForIn { variable_type: Type, variable: String, iterable: Expression, body: Block },
    Function { name: String, parameters: Vec<Parameter>, return_type: Option<Type>, body: Block },
    Import { path: String, alias: Option<String> },
    Terminal { command: String },
    ExpressionStatement { expr: Expression },
}
