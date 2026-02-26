//! # Example 05 - Low-Level Scanner
//!
//! The `Scanner` finds all structural characters (`{`, `}`, `[`, `]`, `:`, `,`, `"`)
//! using SIMD (NEON on ARM, AVX2 on x86) and records their byte offsets into a tape.
//! This is the foundation of zero-copy JSON parsing.
//!
//! Run with: `cargo run --example 05_scanner`

use kowito_json::arena::Scratchpad;
use kowito_json::scanner::Scanner;

fn scan_and_show(label: &str, json: &[u8], tape_capacity: usize) {
    let mut scratchpad = Scratchpad::new(tape_capacity);
    let tape = scratchpad.get_mut_tape();

    let scanner = Scanner::new(json);
    let n = scanner.scan(tape);

    println!("--- {label} ---");
    println!("  Input:  {}", std::str::from_utf8(json).unwrap());

    // Map each offset back to the character it points at
    let tokens: Vec<char> = tape[..n]
        .iter()
        .map(|&offset| json[offset as usize] as char)
        .collect();
    println!("  Tokens: {tokens:?}");
    println!("  Offsets (first {n}): {:?}", &tape[..n]);
    println!();
}

fn main() {
    // Simple object
    scan_and_show("Simple object", br#"{"id":1,"name":"Alice"}"#, 256);

    // Nested object
    scan_and_show("Nested object", br#"{"user":{"id":42,"active":true}}"#, 256);

    // Array of numbers
    scan_and_show("Number array", br#"[1, 2, 3, 42, 100]"#, 256);

    // Array of objects
    scan_and_show("Array of objects", br#"[{"a":1},{"a":2},{"a":3}]"#, 256);

    // Mixed values
    scan_and_show(
        "Mixed",
        br#"{"str":"hello","num":3.14,"bool":true,"null":null,"arr":[1,2]}"#,
        512,
    );

    // String with escaped characters (the scanner still finds `"` boundaries)
    scan_and_show("Escaped string", br#"{"msg":"say \"hi\" and \\wave"}"#, 256);

    // Large-ish input: demonstrate scalability
    let large_json: String = {
        let items: Vec<String> = (0..20)
            .map(|i| format!(r#"{{"id":{i},"val":"item_{i:03}"}}"#))
            .collect();
        format!("[{}]", items.join(","))
    };
    scan_and_show("Large array (20 items)", large_json.as_bytes(), 2048);
}
