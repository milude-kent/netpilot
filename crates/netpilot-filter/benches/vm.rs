use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netpilot_filter::ast::{Expr, Stmt};
use netpilot_filter::attributes::AttributeRegistry;
use netpilot_filter::vm::{FilterContext, FilterVm};

// ── AST builders ────────────────────────────────────────────────────────────

fn int_lit(n: i64) -> Box<Expr> {
    Box::new(Expr::IntLiteral(n))
}

fn var_expr(name: &str) -> Box<Expr> {
    Box::new(Expr::Var(name.to_string()))
}

/// Canned program: if (preference > 100 && metric < 500) then accept; else reject;
/// Represents a typical per-route filter hot path: variable lookup, two
/// comparisons, a logical AND, and a conditional.
fn build_program() -> Stmt {
    let cond = Expr::And(
        Box::new(Expr::Gt(var_expr("preference"), int_lit(100))),
        Box::new(Expr::Lt(var_expr("metric"), int_lit(500))),
    );
    Stmt::If {
        condition: Box::new(cond),
        then_branch: Box::new(Stmt::Accept { expr: None }),
        else_branch: Some(Box::new(Stmt::Reject { expr: None })),
    }
}

// ── Bench functions ─────────────────────────────────────────────────────────

fn bench_vm_filter_program(c: &mut Criterion) {
    let program = build_program();
    let mut ctx = FilterContext::from_route(150, Some(250), "bgp", "192.0.2.0/24");
    let registry = AttributeRegistry::new();

    c.bench_function("vm/filter_program", |b| {
        b.iter(|| {
            let result = FilterVm::evaluate_stmt(
                black_box(&program),
                black_box(&mut ctx),
                black_box(&registry),
            );
            std::hint::black_box(result)
        });
    });
}

fn bench_vm_arithmetic(c: &mut Criterion) {
    // Tight loop: (a + b) * (c - d) — exercises int_op path.
    let expr = Expr::Mul(
        Box::new(Expr::Add(int_lit(10), int_lit(20))),
        Box::new(Expr::Sub(int_lit(50), int_lit(5))),
    );
    let mut ctx = FilterContext::new();
    let registry = AttributeRegistry::new();

    c.bench_function("vm/arithmetic", |b| {
        b.iter(|| {
            let result = FilterVm::evaluate_expr(
                black_box(&expr),
                black_box(&mut ctx),
                black_box(&registry),
            );
            std::hint::black_box(result)
        });
    });
}

criterion_group!(benches, bench_vm_filter_program, bench_vm_arithmetic);
criterion_main!(benches);
