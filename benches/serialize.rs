use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use kowito_json_derive::Kjson;
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Payload A – tiny HTTP-style response (3 fields)
// ---------------------------------------------------------------------------
#[derive(Kjson, Debug, serde::Serialize)]
pub struct MessageResponse {
    pub message: String,
    pub status: i32,
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Payload B – medium user record (7 fields, mixed types)
// ---------------------------------------------------------------------------
#[derive(Kjson, Debug, serde::Serialize)]
pub struct UserRecord {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub age: u32,
    pub score: f64,
    pub verified: bool,
    pub credits: i64,
}

// ---------------------------------------------------------------------------
// Payload C – metrics / telemetry (all numeric, 8 fields)
// ---------------------------------------------------------------------------
#[derive(Kjson, Debug, serde::Serialize)]
pub struct Metrics {
    pub requests: u64,
    pub errors: u64,
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub latency_p99: f64,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    pub timestamp: i64,
}

#[derive(Kjson, Debug, serde::Serialize)]
pub struct LongStringRecord {
    pub id: u64,
    pub data: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
fn tiny_payload() -> MessageResponse {
    MessageResponse {
        message: "Hello, World!".to_string(),
        status: 200,
        success: true,
    }
}

fn medium_payload() -> UserRecord {
    UserRecord {
        id: 123_456_789,
        username: "john_doe".to_string(),
        email: "john.doe@example.com".to_string(),
        age: 32,
        score: 98.765,
        verified: true,
        credits: -42,
    }
}

fn numeric_payload() -> Metrics {
    Metrics {
        requests: 9_876_543,
        errors: 42,
        latency_p50: 1.234,
        latency_p95: 9.876,
        latency_p99: 23.456,
        cpu_pct: 67.3,
        mem_bytes: 1_073_741_824,
        timestamp: 1_740_000_000,
    }
}

fn long_string_payload(size: usize) -> LongStringRecord {
    let mut data = String::with_capacity(size);
    for i in 0..size {
        data.push((b'a' + (i % 26) as u8) as char);
    }
    LongStringRecord { id: 1, data }
}

// ---------------------------------------------------------------------------
// Benchmark helpers – run serde_json / sonic_rs / kowito for one payload
// ---------------------------------------------------------------------------
macro_rules! bench_trio {
    ($group:expr, $id:expr, $val:expr) => {{
        let mut buf = Vec::<u8>::with_capacity(512);

        // Measure serialized byte length for throughput reporting
        $val.to_kbytes(&mut buf);
        $group.throughput(Throughput::Bytes(buf.len() as u64));

        $group.bench_with_input(BenchmarkId::new("serde_json", $id), &$val, |b, v| {
            b.iter(|| {
                buf.clear();
                serde_json::to_writer(&mut buf, v).unwrap();
                black_box(&buf);
            });
        });

        $group.bench_with_input(BenchmarkId::new("sonic_rs", $id), &$val, |b, v| {
            b.iter(|| {
                buf.clear();
                sonic_rs::to_writer(&mut buf, v).unwrap();
                black_box(&buf);
            });
        });

        $group.bench_with_input(BenchmarkId::new("kowito_json_jit", $id), &$val, |b, v| {
            b.iter(|| {
                buf.clear();
                v.to_kbytes(&mut buf);
                black_box(buf.as_slice());
            });
        });
    }};
}

// ---------------------------------------------------------------------------
// Benchmark: Large String Escaping (Tests SIMD)
// ---------------------------------------------------------------------------
fn bench_large_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("Large-String Serialization (SIMD)");

    // 10 KB of safe characters to hit the SIMD fast-path
    bench_trio!(group, "10kb_safe", long_string_payload(10_000));

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Micro-Payload Serialization
// ---------------------------------------------------------------------------
fn bench_micro(c: &mut Criterion) {
    let mut group = c.benchmark_group("Micro-Payload Serialization");

    bench_trio!(group, "tiny_3fields", tiny_payload());
    bench_trio!(group, "medium_7fields", medium_payload());
    bench_trio!(group, "numeric_8fields", numeric_payload());

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Repeated hot-loop (simulates a web-server flushing many responses)
// ---------------------------------------------------------------------------
fn bench_hot_loop(c: &mut Criterion) {
    const N: usize = 1_000;
    let mut group = c.benchmark_group("Hot-Loop Serialization (1 000 items)");

    // Pre-build payloads so allocation is not part of the measured loop
    let items: Vec<UserRecord> = (0..N)
        .map(|i| UserRecord {
            id: i as u64,
            username: format!("user_{i}"),
            email: format!("user_{i}@example.com"),
            age: (20 + i % 60) as u32,
            score: i as f64 * 0.01,
            verified: i % 2 == 0,
            credits: i as i64 - 500,
        })
        .collect();

    let mut buf = Vec::<u8>::with_capacity(256 * N);
    // Measure "output size per batch" for throughput
    for item in &items {
        item.to_kbytes(&mut buf);
    }
    group.throughput(Throughput::Bytes(buf.len() as u64));
    buf.clear();

    group.bench_function("serde_json", |b| {
        b.iter(|| {
            buf.clear();
            for item in &items {
                serde_json::to_writer(&mut buf, item).unwrap();
            }
            black_box(&buf);
        });
    });

    group.bench_function("sonic_rs", |b| {
        b.iter(|| {
            buf.clear();
            for item in &items {
                sonic_rs::to_writer(&mut buf, item).unwrap();
            }
            black_box(&buf);
        });
    });

    group.bench_function("kowito_json_jit", |b| {
        b.iter(|| {
            buf.clear();
            for item in &items {
                item.to_kbytes(&mut buf);
            }
            black_box(buf.as_slice());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_micro, bench_hot_loop, bench_large_string);
criterion_main!(benches);
