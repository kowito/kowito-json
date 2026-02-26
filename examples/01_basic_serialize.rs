//! # Example 01 - Basic Serialization with Derive Macro
//!
//! Demonstrates the simplest way to serialize a Rust struct to JSON
//! using the `#[derive(Kjson)]` macro.
//!
//! Run with: `cargo run --example 01_basic_serialize`

use kowito_json_derive::Kjson;

// Step 1: Derive Kjson on your struct.
// All fields must implement `Serialize` (primitives, String, bool, etc.)
#[derive(Debug, Kjson)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub score: f64,
    pub is_active: bool,
}

fn main() {
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        score: 98.6,
        is_active: true,
    };

    // Step 2: Allocate an output buffer.
    let mut buf = Vec::new();

    // Step 3: Call the generated `to_kbytes` method.
    user.to_kbytes(&mut buf);

    // Step 4: Convert to a UTF-8 string and print.
    let json = std::str::from_utf8(&buf).unwrap();
    println!("Serialized: {json}");
    // Output: {"id":1,"name":"Alice","score":98.6,"is_active":true}

    // Appending to an existing buffer is zero-cost: to_kbytes preserves prior contents.
    let prefix = b"data=";
    let mut out = prefix.to_vec();
    user.to_kbytes(&mut out);
    println!("Appended:   {}", std::str::from_utf8(&out).unwrap());
    // Output: data={"id":1,"name":"Alice","score":98.6,"is_active":true}
}
