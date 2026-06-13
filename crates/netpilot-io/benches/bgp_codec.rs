use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netpilot_io::bgp::{BgpAttribute, BgpMessage, BgpSession};

fn make_update() -> BgpMessage {
    let nh = BgpAttribute {
        flags: 0x40,
        code: 3,
        value: vec![10, 0, 0, 1],
    };
    let origin = BgpAttribute {
        flags: 0x40,
        code: 1,
        value: vec![0],
    };
    let aspath = BgpAttribute {
        flags: 0x40,
        code: 2,
        value: vec![0x02, 0x00],
    };
    BgpSession::build_update(
        vec![],
        vec![nh, origin, aspath],
        vec!["192.0.2.0/24".into(), "198.51.100.0/24".into()],
    )
}

fn bench_encode_update(c: &mut Criterion) {
    let msg = make_update();
    c.bench_function("encode_update", |b| {
        b.iter(|| BgpSession::encode_update(black_box(&msg)));
    });
}

fn bench_decode_message(c: &mut Criterion) {
    let msg = make_update();
    let bytes = BgpSession::encode_update(&msg);
    c.bench_function("decode_message", |b| {
        b.iter(|| BgpSession::decode_message(black_box(&bytes)));
    });
}

criterion_group!(benches, bench_encode_update, bench_decode_message);
criterion_main!(benches);
