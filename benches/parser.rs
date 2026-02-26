use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kowit_json::scanner::Scanner;
use kowit_json::arena::Scratchpad;

fn bench_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("Scanner");
    
    let json_data = br#"{"name": "Kowit", "type": "JSON", "version": 1, "features": ["simd", "zero-copy", "fast"]}"#;
    
    let mut scratchpad = Scratchpad::new(1024);

    group.bench_function("scan_baseline", |b| {
        b.iter(|| {
            let scanner = Scanner::new(black_box(json_data));
            let tape = scratchpad.get_mut_tape();
            let count = scanner.scan(tape);
            black_box(count);
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_scanner);
criterion_main!(benches);
