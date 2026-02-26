//! # Example 04 - Arena / Scratchpad: Zero-Allocation Tape
//!
//! The `Scratchpad` pre-allocates a reusable tape buffer so the scanner
//! never touches the heap in the hot path. The thread-local global scratchpad
//! is the most efficient option for multi-threaded servers.
//!
//! Run with: `cargo run --example 04_arena_scratch`

use kowito_json::KView;
use kowito_json::arena::{Scratchpad, with_scratch_tape};
use kowito_json::scanner::Scanner;

fn main() {
    // -----------------------------------------------------------------------
    // Option A: Manual Scratchpad (explicit lifetime, good for benchmarks)
    // -----------------------------------------------------------------------
    println!("=== Manual Scratchpad ===");
    {
        let mut scratchpad = Scratchpad::new(4096); // 4096 u32 tape slots
        let tape = scratchpad.get_mut_tape();

        let json = br#"{"name":"Kowito","version":2}"#;
        let scanner = Scanner::new(json);
        let n = scanner.scan(tape);

        println!("  JSON: {}", std::str::from_utf8(json).unwrap());
        println!("  Structural tokens found: {n}");
        println!("  Tape (first {n} entries): {:?}", &tape[..n]);
    }

    // -----------------------------------------------------------------------
    // Option B: Thread-Local Global Scratchpad (zero-allocation, production use)
    // -----------------------------------------------------------------------
    println!("\n=== Thread-Local Global Scratchpad ===");
    {
        let jsons = [
            br#"{"a":1}"# as &[u8],
            br#"{"x":true,"y":false}"#,
            br#"[1,2,3,4,5]"#,
            br#"{"nested":{"deep":42}}"#,
        ];

        for json in &jsons {
            with_scratch_tape(|tape| {
                let scanner = Scanner::new(json);
                let n = scanner.scan(tape);
                // Build a KView from the scanned tape
                let _view = KView::new(json, &tape[..n]);
                println!("  {:30} → {} tokens", std::str::from_utf8(json).unwrap(), n);
            });
        }
    }

    // -----------------------------------------------------------------------
    // Option C: Reuse one scratchpad across many parses (batch processing)
    // -----------------------------------------------------------------------
    println!("\n=== Reuse Scratchpad Across Many Parses ===");
    {
        let mut scratchpad = Scratchpad::new(65_536); // 256 KB tape

        let batch = [
            format!(r#"{{"id":{},"val":"x{:04}"}}"#, 1, 1),
            format!(r#"{{"id":{},"val":"x{:04}"}}"#, 2, 2),
            format!(r#"{{"id":{},"val":"x{:04}"}}"#, 3, 3),
        ];

        for (i, json_str) in batch.iter().enumerate() {
            let json = json_str.as_bytes();
            let tape = scratchpad.get_mut_tape(); // reuse same allocation
            let n = Scanner::new(json).scan(tape);
            let _view = KView::new(json, &tape[..n]);
            println!("  [{i}] parsed {n} tokens from `{json_str}`");
        }
    }
}
