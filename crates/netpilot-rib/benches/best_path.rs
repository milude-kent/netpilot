use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use netpilot_rib::route::{NextHop, RouteEntry, RouteKey};
use netpilot_rib::selection::{find_ecmp, select_best};

fn make_candidates(n: usize) -> Vec<RouteEntry> {
    (0..n)
        .map(|i| {
            RouteEntry::new(
                RouteKey::prefix("192.0.2.0/24"),
                "master",
                "bench",
                (i as u32) % 256,
            )
            .with_next_hop(NextHop::new(&format!("10.0.0.{}", i % 256)))
            .with_metric((i as u32) % 1000)
        })
        .collect()
}

fn bench_select_best(c: &mut Criterion) {
    let mut group = c.benchmark_group("select_best");
    for n in [10usize, 100, 1000, 10000] {
        let candidates = make_candidates(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &candidates, |b, c| {
            b.iter(|| select_best(black_box(c.as_slice())));
        });
    }
    group.finish();
}

fn bench_find_ecmp(c: &mut Criterion) {
    let candidates = make_candidates(1000);
    let best = select_best(&candidates).cloned().expect("non-empty");
    c.bench_function("find_ecmp/1000", |b| {
        b.iter(|| find_ecmp(black_box(&candidates), black_box(&best)));
    });
}

criterion_group!(benches, bench_select_best, bench_find_ecmp);
criterion_main!(benches);
