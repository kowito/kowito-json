//! # Example 03 - Advanced Types: Option, Vec, Box, Cow, Nested Structs
//!
//! kowito-json's Serialize trait is implemented for all standard collection
//! wrappers. Mix them freely in your data model.
//!
//! Run with: `cargo run --example 03_advanced_types`

use kowito_json::serialize::Serialize;
use kowito_json_derive::Kjson;
use std::borrow::Cow;

// --- Nested structs ---
#[derive(Debug, Kjson)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip: String,
}

#[derive(Debug, Kjson)]
pub struct Company {
    pub name: String,
    pub employee_count: u32,
}

fn print(label: &str, buf: &[u8]) {
    println!("{label:<16} {}", std::str::from_utf8(buf).unwrap());
}

fn main() {
    let mut buf = Vec::new();

    // --- Option<T>: Some → value, None → null ---
    let some_val: Option<i32> = Some(42);
    let none_val: Option<i32> = None;

    some_val.serialize(&mut buf);
    print("Option Some:", &buf);
    buf.clear();

    none_val.serialize(&mut buf);
    print("Option None:", &buf);
    buf.clear();

    // --- Vec<T> → JSON array ---
    let numbers: Vec<i32> = vec![1, 2, 3, 4, 5];
    numbers.serialize(&mut buf);
    print("Vec<i32>:", &buf);
    buf.clear();

    let names: Vec<String> = vec!["Alice".into(), "Bob".into(), "Carol".into()];
    names.serialize(&mut buf);
    print("Vec<String>:", &buf);
    buf.clear();

    // --- Vec<Option<i32>> ---
    let sparse: Vec<Option<i32>> = vec![Some(1), None, Some(3), None, Some(5)];
    sparse.serialize(&mut buf);
    print("Vec<Option>:", &buf);
    buf.clear();

    // --- Box<T> ---
    let boxed: Box<i64> = Box::new(999);
    boxed.serialize(&mut buf);
    print("Box<i64>:", &buf);
    buf.clear();

    // --- Cow<str> ---
    let borrowed: Cow<str> = Cow::Borrowed("borrowed string");
    borrowed.serialize(&mut buf);
    print("Cow<str> Borrow:", &buf);
    buf.clear();

    let owned: Cow<str> = Cow::Owned("owned string".to_string());
    owned.serialize(&mut buf);
    print("Cow<str> Owned:", &buf);
    buf.clear();

    // --- Slice ---
    let slice: &[u8] = &[10, 20, 30, 40];
    slice.serialize(&mut buf);
    print("&[u8] slice:", &buf);
    buf.clear();

    // --- Nested: Address uses to_kbytes from derive macro ---
    let addr = Address {
        street: "123 Main St".to_string(),
        city: "San Francisco".to_string(),
        zip: "94102".to_string(),
    };
    addr.to_kbytes(&mut buf);
    print("Address:", &buf);
    buf.clear();

    // --- Derived struct that holds a nested derived struct via to_kbytes ---
    // Note: nested struct fields work when they implement Serialize
    let company = Company {
        name: "Kowito Inc.".to_string(),
        employee_count: 42,
    };
    company.to_kbytes(&mut buf);
    print("Company:", &buf);
    buf.clear();
}
