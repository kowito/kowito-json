//! Example 09 — serde_json-compatible serialization with kowito-json.
//!
//! Run with:  cargo run --example 09_serde_compat

use kowito_json::KJson;
use std::collections::HashMap;

// ── Named struct (schema-JIT fast path) ─────────────────────────────────────

#[derive(KJson, Debug)]
struct User {
    pub id: u64,
    pub name: String,
    pub active: bool,
    pub score: f64,
}

// ── Field attrs ──────────────────────────────────────────────────────────────

#[derive(KJson, Debug)]
struct ApiEvent {
    #[kjson(rename = "eventId")]
    pub id: u64,
    pub kind: String,
    #[kjson(skip)]
    pub _trace_id: String,
}

// ── Newtype + tuple struct ────────────────────────────────────────────────────

#[derive(KJson, Debug)]
struct UserId(u64);

#[derive(KJson, Debug)]
struct Point(f64, f64);

// ── Unit struct ───────────────────────────────────────────────────────────────

#[derive(KJson, Debug)]
struct Tombstone;

// ── Enum ─────────────────────────────────────────────────────────────────────

#[derive(KJson, Debug)]
enum Event {
    Ping,
    Message(String),
    Resize(u32, u32),
    Move { x: i32, y: i32 },
}

// ── Arbitrary serde::Serialize type (no KJson derive) ─────────────────────────

#[derive(serde::Serialize)]
struct Config {
    host: String,
    port: u16,
    tags: Vec<String>,
    meta: HashMap<String, String>,
}

fn main() {
    // Named struct — to_json_bytes (schema-JIT)
    let user = User { id: 1, name: "Alice".into(), active: true, score: 98.6 };
    let mut buf = Vec::new();
    user.to_json_bytes(&mut buf);
    println!("[KJson fast]  {}", std::str::from_utf8(&buf).unwrap());

    // Named struct — to_string (serde path)
    println!("[serde path]  {}", kowito_json::to_string(&user).unwrap());

    // Field attrs
    let ev = ApiEvent { id: 42, kind: "click".into(), _trace_id: "secret".into() };
    let mut buf2 = Vec::new();
    ev.to_json_bytes(&mut buf2);
    println!("[rename/skip] {}", std::str::from_utf8(&buf2).unwrap());

    // Newtype
    println!("[newtype]     {}", kowito_json::to_string(&UserId(7)).unwrap());

    // Tuple struct
    println!("[tuple]       {}", kowito_json::to_string(&Point(1.0, 2.0)).unwrap());

    // Unit struct
    println!("[unit]        {}", kowito_json::to_string(&Tombstone).unwrap());

    // Enums
    for e in [
        Event::Ping,
        Event::Message("hello".into()),
        Event::Resize(1920, 1080),
        Event::Move { x: 10, y: -5 },
    ] {
        let mut b = Vec::new();
        e.to_json_bytes(&mut b);
        println!("[enum]        {}", std::str::from_utf8(&b).unwrap());
    }

    // Any serde::Serialize type (HashMap, etc.)
    let mut meta = HashMap::new();
    meta.insert("env".into(), "prod".into());
    let cfg = Config {
        host: "localhost".into(),
        port: 8080,
        tags: vec!["web".into(), "api".into()],
        meta,
    };
    println!("[serde compat] {}", kowito_json::to_string(&cfg).unwrap());

    // Pretty-print
    println!("\n[pretty]\n{}", kowito_json::to_string_pretty(&cfg).unwrap());
}
