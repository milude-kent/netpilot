use routeplane_filter::ast::*;

#[test]
fn for_loop_over_int_set() {
    let stmt = Stmt::For {
        var_type: Some("int".to_string()),
        var_name: "v".to_string(),
        expr: Box::new(Expr::Var("my_set".to_string())),
        body: Box::new(Stmt::Accept { expr: None }),
    };
    assert!(matches!(stmt, Stmt::For { .. }));
}

#[test]
fn case_statement_with_set_branches() {
    let stmt = Stmt::Case {
        expr: Box::new(Expr::PrefixField(PrefixField::Type)),
        branches: vec![CaseBranch {
            set: Some(Expr::Var("NET_IP4".into())),
            stmt: Box::new(Stmt::Accept { expr: None }),
        }],
        else_branch: Some(Box::new(Stmt::Reject { expr: None })),
    };
    assert!(matches!(stmt, Stmt::Case { .. }));
}

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
