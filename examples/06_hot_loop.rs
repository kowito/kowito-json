//! # Example 06 - Hot-Loop Batch Serialization
//!
//! Kowito's schema-JIT serializer shines when you serialize many structs in
//! a tight loop (e.g., building a JSON-lines stream or a large JSON array).
//!
//! Key techniques:
//!   - Pre-reserve the output buffer for the whole batch
//!   - Call `to_kbytes` in a loop — no allocations in the hot path
//!
//! Run with: `cargo run --example 06_hot_loop`

use kowito_json_derive::Kjson;

#[derive(Debug, Kjson)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
    pub request_id: u64,
    pub latency_us: u32,
}

#[derive(Debug, Kjson)]
pub struct MetricPoint {
    pub name: String,
    pub value: f64,
    pub ts: u64,
}

fn main() {
    // -----------------------------------------------------------------------
    // Example A: JSON-Lines format (one JSON object per line)
    // -----------------------------------------------------------------------
    println!("=== JSON-Lines Stream ===");
    {
        let entries: Vec<LogEntry> = (0..5)
            .map(|i| LogEntry {
                timestamp: 1_700_000_000 + i,
                level: if i % 2 == 0 {
                    "INFO".into()
                } else {
                    "WARN".into()
                },
                message: format!("Request completed in {}ms", i * 10 + 5),
                request_id: 1000 + i,
                latency_us: (i as u32) * 1000 + 250,
            })
            .collect();

        let mut buf = Vec::with_capacity(entries.len() * 128);
        for entry in &entries {
            entry.to_kbytes(&mut buf);
            buf.push(b'\n'); // newline delimiter
        }

        println!("{}", std::str::from_utf8(&buf).unwrap());
    }

    // -----------------------------------------------------------------------
    // Example B: JSON Array of structs
    // -----------------------------------------------------------------------
    println!("=== JSON Array ===");
    {
        let points: Vec<MetricPoint> = vec![
            MetricPoint {
                name: "cpu_pct".into(),
                value: 23.5,
                ts: 1000,
            },
            MetricPoint {
                name: "mem_mb".into(),
                value: 1024.0,
                ts: 1001,
            },
            MetricPoint {
                name: "rps".into(),
                value: 9871.3,
                ts: 1002,
            },
            MetricPoint {
                name: "p99_ms".into(),
                value: 4.7,
                ts: 1003,
            },
        ];

        // Pre-reserve for whole array (avoids reallocations)
        let mut buf = Vec::with_capacity(points.len() * 80 + 2);
        buf.push(b'[');
        for (i, pt) in points.iter().enumerate() {
            if i > 0 {
                buf.push(b',');
            }
            pt.to_kbytes(&mut buf);
        }
        buf.push(b']');

        println!("{}", std::str::from_utf8(&buf).unwrap());
    }

    // -----------------------------------------------------------------------
    // Example C: Reuse buffer across requests (server hot-path pattern)
    // -----------------------------------------------------------------------
    println!("\n=== Reused Buffer (Server Pattern) ===");
    {
        // Imagine this buf lives in a request handler, cleared and reused
        let mut buf: Vec<u8> = Vec::with_capacity(512);

        let metrics = [
            ("latency_p50", 1.2_f64),
            ("latency_p99", 8.7_f64),
            ("error_rate", 0.003_f64),
        ];

        for (name, val) in metrics {
            buf.clear(); // clear but keep allocation
            let pt = MetricPoint {
                name: name.into(),
                value: val,
                ts: 9999,
            };
            pt.to_kbytes(&mut buf);
            // Simulate sending over a wire
            println!("  → {}", std::str::from_utf8(&buf).unwrap());
        }
    }
}
