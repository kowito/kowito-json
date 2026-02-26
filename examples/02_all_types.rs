//! # Example 02 - All Supported Primitive Types
//!
//! Shows every type that KJson supports natively for serialization.
//!
//! Run with: `cargo run --example 02_all_types`

use kowito_json_derive::KJson;

#[derive(Debug, KJson)]
pub struct Integers {
    pub a: i8,
    pub b: i16,
    pub c: i32,
    pub d: i64,
    pub e: isize,
    pub f: u8,
    pub g: u16,
    pub h: u32,
    pub i: u64,
    pub j: usize,
}

#[derive(Debug, KJson)]
pub struct Floats {
    pub x: f32,
    pub y: f64,
}

#[derive(Debug, KJson)]
pub struct Strings {
    pub plain: String,
    pub with_quotes: String,
    pub with_newline: String,
    pub with_tab: String,
    pub with_backslash: String,
    pub unicode_control: String,
}

#[derive(Debug, KJson)]
pub struct Booleans {
    pub yes: bool,
    pub no: bool,
}

fn print(label: &str, buf: &[u8]) {
    println!("{label}: {}", std::str::from_utf8(buf).unwrap());
}

fn main() {
    let mut buf = Vec::new();

    // --- Integers ---
    let ints = Integers {
        a: i8::MIN,
        b: i16::MAX,
        c: -100_000,
        d: i64::MAX,
        e: 42,
        f: u8::MAX,
        g: 65535,
        h: 4_000_000_000,
        i: u64::MAX,
        j: 1024,
    };
    ints.to_json_bytes(&mut buf);
    print("Integers", &buf);
    buf.clear();

    // --- Floats ---
    // Kowito uses `ryu` (Grisu3/Dragon4): shortest round-trip representation.
    let floats = Floats {
        x: std::f32::consts::PI,
        y: std::f64::consts::PI,
    };
    floats.to_json_bytes(&mut buf);
    print("Floats  ", &buf);
    buf.clear();

    // --- Strings: all escape sequences ---
    let strings = Strings {
        plain: "hello world".to_string(),
        with_quotes: r#"say "hi""#.to_string(),
        with_newline: "line1\nline2".to_string(),
        with_tab: "col1\tcol2".to_string(),
        with_backslash: "C:\\Users\\Alice".to_string(),
        unicode_control: "\x01\x1F".to_string(), // serialized as \u0001\u001f
    };
    strings.to_json_bytes(&mut buf);
    print("Strings ", &buf);
    buf.clear();

    // --- Booleans ---
    let bools = Booleans {
        yes: true,
        no: false,
    };
    bools.to_json_bytes(&mut buf);
    print("Booleans", &buf);
}
