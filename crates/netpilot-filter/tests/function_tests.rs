use netpilot_filter::ast::*;

#[test]
fn filter_function_with_return_type() {
    let func = FilterFunction {
        name: "is_bogon".into(),
        params: vec![("p".into(), Some("prefix".into()))],
        return_type: Some("bool".into()),
        locals: vec![],
        body: Stmt::Return(Some(Expr::BoolLiteral(true))),
    };
    assert_eq!(func.return_type.as_deref(), Some("bool"));
}

#[test]
fn function_with_local_variables() {
    let func = FilterFunction {
        name: "count".into(),
        params: vec![("p".into(), Some("bgppath".into()))],
        return_type: Some("int".into()),
        locals: vec![("n".into(), "int".into())],
        body: Stmt::Return(Some(Expr::IntLiteral(0))),
    };
    assert_eq!(func.locals.len(), 1);
}

#[test]
fn validate_types_rejects_unknown_return() {
    let func = FilterFunction {
        name: "bad".into(),
        params: vec![],
        return_type: Some("garbage".into()),
        locals: vec![],
        body: Stmt::Accept { expr: None },
    };
    assert!(func.validate_types().is_err());
}

#[test]
fn validate_types_accepts_valid() {
    let func = FilterFunction {
        name: "good".into(),
        params: vec![("x".into(), Some("bgppath".into()))],
        return_type: Some("bool".into()),
        locals: vec![],
        body: Stmt::Return(Some(Expr::BoolLiteral(true))),
    };
    assert!(func.validate_types().is_ok());
}
