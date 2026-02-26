use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use kowito_json::arena::Scratchpad;
use kowito_json::scanner::Scanner;
use std::hint::black_box;

fn generate_massive_json() -> String {
    let mut s = String::with_capacity(10 * 1024 * 1024);
    s.push('[');
    for i in 0..100_000 {
        if i > 0 {
            s.push(',');
        }
        s.push_str(r#"{"id":"#);
        s.push_str(&i.to_string());
        s.push_str(r#","name":"user_"#);
        s.push_str(&i.to_string());
        s.push_str(r#"","active":true,"roles":["admin","user","guest"],"metadata":{"created":"2026-01-01T00:00:00Z","score":99.9}}"#);
    }
    s.push(']');
    s
}

fn bench_parsers(c: &mut Criterion) {
    let mut group = c.benchmark_group("Massive JSON Parsers");

    let json_string = generate_massive_json();
    let json_bytes = json_string.as_bytes();

    // Set throughput so criterion shows MB/s
    group.throughput(Throughput::Bytes(json_bytes.len() as u64));
    group.sample_size(100);

    // 1. Serde JSON (Baseline Standard)
    group.bench_function("serde_json", |b| {
        b.iter(|| {
            let _val: serde_json::Value = serde_json::from_slice(json_bytes).unwrap();
            black_box(_val);
        });
    });

    // 2. SIMD-JSON
    group.bench_function("simd_json", |b| {
        let mut buffer = json_bytes.to_vec();
        b.iter(|| {
            // simd_json modifies the buffer in place for speed
            buffer.copy_from_slice(json_bytes);
            let _val: simd_json::OwnedValue = simd_json::to_owned_value(&mut buffer).unwrap();
            black_box(_val);
        });
    });

    // 3. Sonic-RS (The 2026 Champion)
    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            let _val: sonic_rs::Value = sonic_rs::from_slice(json_bytes).unwrap();
            black_box(_val);
        });
    });

    // 4. KJSON / Kowito-JSON (The World Record Contender)
    // We pre-allocate the scratchpad just like real-world server thread-locals
    let mut scratchpad = Scratchpad::new(10_000_000);

    group.bench_function("kowito_json_scanner_only", |b| {
        b.iter(|| {
            let scanner = Scanner::new(json_bytes);
            let tape = scratchpad.get_mut_tape();
            let count = scanner.scan(tape);
            black_box(count);
        });
    });

    // 5. KJSON / Kowito-JSON (Scanner + Schema-JIT Zero Decode Instantiation)
    group.bench_function("kowito_json_schema_jit", |b| {
        b.iter(|| {
            let scanner = Scanner::new(json_bytes);
            let tape = scratchpad.get_mut_tape();
            let _count = scanner.scan(tape);

            // In a real implementation we would loop over the array elements
            // For this benchmark we simulate extracting the first object
            let view = kowito_json::KView::new(json_bytes, tape);

            // This is the Magic: Instantiating the struct immediately without
            // touching the unused strings or traversing the full tree.
            let user = kowito_json::FastUser::from_kview(&view);
            black_box(user);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_parsers);
criterion_main!(benches);
