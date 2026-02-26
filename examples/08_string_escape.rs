//! # Example 08 - String Escaping & SIMD Fast Path
//!
//! Demonstrates the SIMD-accelerated string writer directly.
//! Use `write_str_escape` whenever you need to emit a safe JSON string
//! from raw bytes without going through a full struct serialization.
//!
//! Run with: `cargo run --example 08_string_escape`

use kowito_json::serialize::write_str_escape;

fn show(label: &str, s: &str) {
    let mut buf = Vec::new();
    write_str_escape(&mut buf, s.as_bytes());
    println!("{label:<30} → {}", std::str::from_utf8(&buf).unwrap());
}

fn main() {
    // Plain ASCII – bulk SIMD copy, zero escaping
    show("Plain ASCII", "Hello, World!");

    // Quotes inside a string
    show("Embedded quotes", r#"She said "hello""#);

    // Backslash
    show("Backslash", r"C:\Users\Alice\Documents");

    // Control characters
    show("Newline", "line one\nline two");
    show("Tab", "col1\tcol2\tcol3");
    show("Carriage return", "before\rafter");
    show("Backspace", "ab\x08c");
    show("Form feed", "page\x0Cbreak");

    // Low control bytes (0x00–0x1F) → \u00XX
    show("Null byte", "\x00");
    show("STX control", "\x02");
    show("Unit separator", "\x1F");

    // Mixed: mostly safe, one escape mid-string
    show("Mixed mid-escape", "aaaaaaaaaaaaaaaa\"end");

    // Long string that exercises SIMD fast-path (no escapes for entire blocks)
    let long_safe = "a".repeat(1024);
    let mut buf = Vec::new();
    write_str_escape(&mut buf, long_safe.as_bytes());
    println!(
        "{:<30} → {} bytes (starts: {}, ends: {})",
        "1024-char safe string",
        buf.len(),
        std::str::from_utf8(&buf[..3]).unwrap(),
        std::str::from_utf8(&buf[buf.len() - 3..]).unwrap(),
    );

    // Long string with one escape at the very end
    let long_with_trailing_quote = format!("{}\"", "b".repeat(1024));
    buf.clear();
    write_str_escape(&mut buf, long_with_trailing_quote.as_bytes());
    println!(
        "{:<30} → {} bytes (tail: {})",
        "1024-char + trailing quote",
        buf.len(),
        std::str::from_utf8(&buf[buf.len() - 4..]).unwrap(),
    );
}
