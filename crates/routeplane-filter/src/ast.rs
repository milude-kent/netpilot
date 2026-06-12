// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 RoutePlane Contributors

/// AST for the RoutePlane filter language (BIRD-compatible).
///
/// Covers expressions, statements, filter functions, and the control-flow
/// constructs (for-loop, case-statement) needed by milestone 1.

/// A filter-language expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    BoolLiteral(bool),
    IntLiteral(i64),
    StringLiteral(String),
    IpLiteral(String),
    PrefixLiteral(String),
    SetLiteral(SetLiteral),
    Var(String),

    // arithmetic
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),

    // comparison
    Eq(Box<Expr>, Box<Expr>),
    Neq(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Lte(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Gte(Box<Expr>, Box<Expr>),

    // logical
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),

    // string concatenation
    Concat(Box<Expr>, Box<Expr>),

    // set membership
    Match(Box<Expr>, Box<Expr>),
    NotMatch(Box<Expr>, Box<Expr>),

    // unary
    Not(Box<Expr>),
    Neg(Box<Expr>),

    // member access  expr.field
    Dot(Box<Expr>, String),

    // prefix / bgp-path pseudo-fields
    PrefixField(PrefixField),
    BgpPathField(BgpPathField),

    // function / filter call
    Call { name: String, args: Vec<Expr> },
}

/// A statement (action or control-flow construct).
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assign {
        lhs: String,
        rhs: Box<Expr>,
    },
    Compound(Vec<Stmt>),
    If {
        condition: Box<Expr>,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    For {
        var_type: Option<String>,
        var_name: String,
        expr: Box<Expr>,
        body: Box<Stmt>,
    },
    Case {
        expr: Box<Expr>,
        branches: Vec<CaseBranch>,
        else_branch: Option<Box<Stmt>>,
    },
    Accept {
        expr: Option<Box<Expr>>,
    },
    Reject {
        expr: Option<Box<Expr>>,
    },
    /// Return from a filter function.  The expression, when present, is the
    /// return value.
    Return(Option<Expr>),
    Print(Box<Expr>),
}

/// A top-level filter function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterFunction {
    pub name: String,
    /// (parameter-name, optional-type-name) pairs.
    pub params: Vec<(String, Option<String>)>,
    pub return_type: Option<String>,
    /// (name, type-name) pairs for local variables.
    pub locals: Vec<(String, String)>,
    pub body: Stmt,
}

/// One branch of a `case` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseBranch {
    /// The set expression this branch matches against (the `case` scrutinee is
    /// tested with `~` / element-of).
    pub set: Option<Expr>,
    pub stmt: Box<Stmt>,
}

/// An inline set literal, e.g. `[1, 2, 3]` or `[NET_IP4, NET_IP6]`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetLiteral {
    pub items: Vec<Expr>,
}

/// Pseudo-fields on a prefix value (e.g. `net.type`, `net.ip`, `net.len`).
#[derive(Debug, Clone, PartialEq)]
pub enum PrefixField {
    Net,
    Ip,
    Len,
    Type,
}

/// Pseudo-fields on a BGP path value (e.g. `bgp_path.len`, `bgp_path.first`).
#[derive(Debug, Clone, PartialEq)]
pub enum BgpPathField {
    Length,
    FirstHop,
    NextHop,
}

impl FilterFunction {
    pub fn validate_types(&self) -> Result<(), String> {
        if let Some(ret) = &self.return_type {
            if !is_valid_type_name(ret) {
                return Err(format!("unknown return type: {ret}"));
            }
        }
        for (_, type_opt) in &self.params {
            if let Some(t) = type_opt {
                if !is_valid_type_name(t) {
                    return Err(format!("unknown parameter type: {t}"));
                }
            }
        }
        Ok(())
    }
}

fn is_valid_type_name(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "int"
            | "pair"
            | "quad"
            | "string"
            | "bytestring"
            | "ip"
            | "mac"
            | "prefix"
            | "rd"
            | "ec"
            | "lc"
            | "bgppath"
            | "bgpmask"
            | "clist"
            | "eclist"
            | "lclist"
            | "int set"
            | "prefix set"
            | "pair set"
            | "ec set"
            | "lc set"
    )
}
