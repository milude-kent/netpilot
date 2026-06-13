// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 RoutePlane Contributors

/// Filter VM — evaluates filter expressions and statements against a context
/// that holds route attributes as FilterValues.

use crate::ast::{Expr, Stmt};
use crate::attributes::AttributeRegistry;
use crate::value::FilterValue;
use std::collections::HashMap;

// ── Context ──────────────────────────────────────────────────────────────────

/// Runtime context for filter evaluation — holds route attributes and local
/// variables as FilterValues.
#[derive(Clone, Debug, Default)]
pub struct FilterContext {
    pub attributes: HashMap<String, FilterValue>,
    pub locals: HashMap<String, FilterValue>,
}

impl FilterContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Populate context from route fields.
    pub fn from_route(preference: u32, metric: Option<u32>, source: &str, prefix: &str) -> Self {
        let mut attrs = HashMap::new();
        attrs.insert("preference".into(), FilterValue::Int(preference));
        attrs.insert("metric".into(), FilterValue::Int(metric.unwrap_or(0)));
        attrs.insert("source".into(), FilterValue::String(source.to_string()));
        attrs.insert("net".into(), FilterValue::String(prefix.to_string()));
        Self {
            attributes: attrs,
            locals: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&FilterValue> {
        self.locals
            .get(name)
            .or_else(|| self.attributes.get(name))
    }

    pub fn set(&mut self, name: &str, value: FilterValue) {
        self.locals.insert(name.to_string(), value);
    }
}

// ── VM ───────────────────────────────────────────────────────────────────────

pub struct FilterVm;

impl FilterVm {
    // ── Expression evaluation ────────────────────────────────────────────────

    /// Evaluate a filter expression in the given context.
    /// Returns the result FilterValue, or an error string.
    pub fn evaluate_expr(
        expr: &Expr,
        ctx: &mut FilterContext,
        registry: &AttributeRegistry,
    ) -> Result<FilterValue, String> {
        match expr {
            // ── Literals ─────────────────────────────────────────────────
            Expr::BoolLiteral(b) => Ok(FilterValue::Bool(*b)),
            Expr::IntLiteral(n) => Ok(FilterValue::Int(*n as u32)),
            Expr::StringLiteral(s) => Ok(FilterValue::String(s.clone())),
            Expr::IpLiteral(ip) => Ok(FilterValue::String(ip.clone())),
            Expr::PrefixLiteral(pfx) => Ok(FilterValue::String(pfx.clone())),
            Expr::SetLiteral(_set) => Err("set literal eval not implemented".into()),

            // ── Variables ────────────────────────────────────────────────
            Expr::Var(name) => ctx
                .get(name)
                .cloned()
                .or_else(|| registry.read(name).ok())
                .ok_or_else(|| format!("unknown variable: {name}")),

            // ── Arithmetic ───────────────────────────────────────────────
            Expr::Add(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                int_op(&lv, &rv, |a, b| a.wrapping_add(b))
            }
            Expr::Sub(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                int_op(&lv, &rv, |a, b| a.wrapping_sub(b))
            }
            Expr::Mul(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                int_op(&lv, &rv, |a, b| a.wrapping_mul(b))
            }
            Expr::Div(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                match (&lv, &rv) {
                    (FilterValue::Int(a), FilterValue::Int(b)) => {
                        if *b == 0 {
                            Err("division by zero".into())
                        } else {
                            Ok(FilterValue::Int(a / b))
                        }
                    }
                    _ => Err("division requires int operands".into()),
                }
            }

            // ── Comparison ───────────────────────────────────────────────
            Expr::Eq(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                Ok(FilterValue::Bool(lv == rv))
            }
            Expr::Neq(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                Ok(FilterValue::Bool(lv != rv))
            }
            Expr::Lt(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                cmp_values(&lv, &rv).map(|o| FilterValue::Bool(o == std::cmp::Ordering::Less))
            }
            Expr::Lte(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                cmp_values(&lv, &rv)
                    .map(|o| FilterValue::Bool(o != std::cmp::Ordering::Greater))
            }
            Expr::Gt(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                cmp_values(&lv, &rv)
                    .map(|o| FilterValue::Bool(o == std::cmp::Ordering::Greater))
            }
            Expr::Gte(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                cmp_values(&lv, &rv)
                    .map(|o| FilterValue::Bool(o != std::cmp::Ordering::Less))
            }

            // ── Logical ──────────────────────────────────────────────────
            Expr::And(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                if !Self::truthy(&lv) {
                    return Ok(FilterValue::Bool(false));
                }
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                Ok(FilterValue::Bool(Self::truthy(&rv)))
            }
            Expr::Or(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                if Self::truthy(&lv) {
                    return Ok(FilterValue::Bool(true));
                }
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                Ok(FilterValue::Bool(Self::truthy(&rv)))
            }

            // ── Unary ────────────────────────────────────────────────────
            Expr::Not(inner) => {
                let v = Self::evaluate_expr(inner, ctx, registry)?;
                Ok(FilterValue::Bool(!Self::truthy(&v)))
            }
            Expr::Neg(inner) => {
                let v = Self::evaluate_expr(inner, ctx, registry)?;
                match v {
                    FilterValue::Int(n) => Ok(FilterValue::Int(0u32.wrapping_sub(n))),
                    _ => Err("negation requires int".into()),
                }
            }

            // ── String concat ────────────────────────────────────────────
            Expr::Concat(l, r) => {
                let lv = Self::evaluate_expr(l, ctx, registry)?;
                let rv = Self::evaluate_expr(r, ctx, registry)?;
                Ok(FilterValue::String(format!("{lv}{rv}")))
            }

            // ── Function call ────────────────────────────────────────────
            Expr::Call { name, args } => {
                let vals: Result<Vec<FilterValue>, String> = args
                    .iter()
                    .map(|a| Self::evaluate_expr(a, ctx, registry))
                    .collect();
                Self::call_builtin(name, &vals?, registry)
            }

            // ── Member access ────────────────────────────────────────────
            Expr::Dot(_, _) => Err("member access not implemented".into()),

            // ── Set membership ───────────────────────────────────────────
            Expr::Match(_, _) => Err("set membership not implemented".into()),
            Expr::NotMatch(_, _) => Err("set non-membership not implemented".into()),

            // ── Pseudo-fields ────────────────────────────────────────────
            Expr::PrefixField(_) => Err("prefix field not implemented".into()),
            Expr::BgpPathField(_) => Err("bgp_path field not implemented".into()),
        }
    }

    // ── Statement evaluation ─────────────────────────────────────────────────

    /// Evaluate a filter statement in the given context.
    /// Returns an optional FilterValue (e.g. for `Return`, `Accept`, `Reject`).
    pub fn evaluate_stmt(
        stmt: &Stmt,
        ctx: &mut FilterContext,
        registry: &AttributeRegistry,
    ) -> Result<Option<FilterValue>, String> {
        match stmt {
            Stmt::Assign { lhs, rhs } => {
                let v = Self::evaluate_expr(rhs, ctx, registry)?;
                ctx.set(lhs, v.clone());
                Ok(Some(v))
            }
            Stmt::Compound(stmts) => {
                let mut last = None;
                for s in stmts {
                    last = Self::evaluate_stmt(s, ctx, registry)?;
                }
                Ok(last)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let c = Self::evaluate_expr(condition, ctx, registry)?;
                if Self::truthy(&c) {
                    Self::evaluate_stmt(then_branch, ctx, registry)
                } else if let Some(else_s) = else_branch {
                    Self::evaluate_stmt(else_s, ctx, registry)
                } else {
                    Ok(None)
                }
            }
            Stmt::Accept { expr } => match expr {
                Some(e) => {
                    let v = Self::evaluate_expr(e, ctx, registry)?;
                    Ok(Some(v))
                }
                None => Ok(Some(FilterValue::Bool(true))),
            },
            Stmt::Reject { expr } => match expr {
                Some(e) => {
                    let v = Self::evaluate_expr(e, ctx, registry)?;
                    Ok(Some(v))
                }
                None => Ok(Some(FilterValue::Bool(false))),
            },
            Stmt::Return(expr) => match expr {
                Some(e) => {
                    let v = Self::evaluate_expr(e, ctx, registry)?;
                    Ok(Some(v))
                }
                None => Ok(None),
            },
            Stmt::Print(expr) => {
                let v = Self::evaluate_expr(expr, ctx, registry)?;
                eprintln!("filter: {v}");
                Ok(Some(v))
            }

            Stmt::For { .. } => Err("for loops not implemented".into()),
            Stmt::Case { .. } => Err("case statements not implemented".into()),
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn truthy(val: &FilterValue) -> bool {
        match val {
            FilterValue::Bool(b) => *b,
            FilterValue::Int(n) => *n != 0,
            FilterValue::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    fn call_builtin(
        name: &str,
        args: &[FilterValue],
        registry: &AttributeRegistry,
    ) -> Result<FilterValue, String> {
        match name {
            "defined" => {
                if let Some(FilterValue::String(s)) = args.first() {
                    Ok(FilterValue::Bool(registry.is_defined(s)))
                } else {
                    Ok(FilterValue::Bool(false))
                }
            }
            "print" => {
                let output = crate::builtins::print(args);
                eprintln!("{output}");
                Ok(FilterValue::Bool(true))
            }
            "printn" => {
                let output = crate::builtins::printn(args);
                eprintln!("{output}");
                Ok(FilterValue::Bool(true))
            }
            "len" => match args.first() {
                Some(FilterValue::String(s)) => Ok(FilterValue::Int(s.len() as u32)),
                _ => Err("len: unsupported argument".into()),
            },
            _ => Err(format!("unknown function: {name}")),
        }
    }
}

// ── Arithmetic helpers ───────────────────────────────────────────────────────

fn int_op<F>(l: &FilterValue, r: &FilterValue, f: F) -> Result<FilterValue, String>
where
    F: FnOnce(u32, u32) -> u32,
{
    match (l, r) {
        (FilterValue::Int(a), FilterValue::Int(b)) => Ok(FilterValue::Int(f(*a, *b))),
        _ => Err("arithmetic requires int operands".into()),
    }
}

// ── Comparison helpers ───────────────────────────────────────────────────────

fn cmp_values(l: &FilterValue, r: &FilterValue) -> Result<std::cmp::Ordering, String> {
    match (l, r) {
        (FilterValue::Int(a), FilterValue::Int(b)) => Ok(a.cmp(b)),
        (FilterValue::String(a), FilterValue::String(b)) => Ok(a.cmp(b)),
        _ => Err("comparison requires same-type operands".into()),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    fn int_lit(n: i64) -> Box<Expr> {
        Box::new(Expr::IntLiteral(n))
    }

    fn var_expr(name: &str) -> Box<Expr> {
        Box::new(Expr::Var(name.to_string()))
    }

    #[test]
    fn vm_evaluates_literal() {
        let expr = Expr::IntLiteral(42);
        let mut ctx = FilterContext::new();
        let reg = AttributeRegistry::new();
        let result = FilterVm::evaluate_expr(&expr, &mut ctx, &reg).unwrap();
        assert_eq!(result, FilterValue::Int(42));
    }

    #[test]
    fn vm_evaluates_binary_arithmetic() {
        let expr = Expr::Add(int_lit(10), int_lit(5));
        let mut ctx = FilterContext::new();
        let reg = AttributeRegistry::new();
        let result = FilterVm::evaluate_expr(&expr, &mut ctx, &reg).unwrap();
        assert_eq!(result, FilterValue::Int(15));
    }

    #[test]
    fn vm_evaluates_comparison() {
        let expr = Expr::Gt(int_lit(100), int_lit(50));
        let mut ctx = FilterContext::new();
        let reg = AttributeRegistry::new();
        let result = FilterVm::evaluate_expr(&expr, &mut ctx, &reg).unwrap();
        assert_eq!(result, FilterValue::Bool(true));
    }

    #[test]
    fn vm_evaluates_conditional() {
        // Stmt::If: if 10 > 5 then result=1 else result=0
        let cond = Expr::Gt(int_lit(10), int_lit(5));
        let stmt = Stmt::If {
            condition: Box::new(cond),
            then_branch: Box::new(Stmt::Assign {
                lhs: "result".into(),
                rhs: int_lit(1),
            }),
            else_branch: Some(Box::new(Stmt::Assign {
                lhs: "result".into(),
                rhs: int_lit(0),
            })),
        };
        let mut ctx = FilterContext::new();
        let reg = AttributeRegistry::new();
        FilterVm::evaluate_stmt(&stmt, &mut ctx, &reg).unwrap();
        assert_eq!(ctx.get("result").unwrap(), &FilterValue::Int(1));
    }

    #[test]
    fn vm_evaluates_route_context() {
        let mut ctx = FilterContext::from_route(100, Some(10), "bgp", "10.0.0.0/8");
        let expr = Expr::Var("preference".into());
        let reg = AttributeRegistry::new();
        let result = FilterVm::evaluate_expr(&expr, &mut ctx, &reg).unwrap();
        assert_eq!(result, FilterValue::Int(100));
    }

    #[test]
    fn vm_filter_accepts_bgp_with_high_preference() {
        let mut ctx = FilterContext::from_route(100, Some(10), "bgp", "10.0.0.0/8");
        // Expression: preference > 50 && metric < 100
        let expr = Expr::And(
            Box::new(Expr::Gt(var_expr("preference"), int_lit(50))),
            Box::new(Expr::Lt(var_expr("metric"), int_lit(100))),
        );
        let reg = AttributeRegistry::new();
        let result = FilterVm::evaluate_expr(&expr, &mut ctx, &reg).unwrap();
        assert_eq!(result, FilterValue::Bool(true));
    }

    #[test]
    fn vm_evaluates_assignment() {
        let stmt = Stmt::Assign {
            lhs: "x".into(),
            rhs: Box::new(Expr::Add(int_lit(3), int_lit(4))),
        };
        let mut ctx = FilterContext::new();
        let reg = AttributeRegistry::new();
        FilterVm::evaluate_stmt(&stmt, &mut ctx, &reg).unwrap();
        assert_eq!(ctx.get("x").unwrap(), &FilterValue::Int(7));
    }
}
