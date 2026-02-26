# kjson

**The fastest JSON parser on Earth.**

`kjson` is a cutting-edge JSON parsing and binding library for Rust. Leveraging state-of-the-art SIMD instructions, hardware prefetching, and a "Zero-Decode" philosophy, it parses JSON by completely avoiding scalar loops and allocations until absolutely necessary.

Currently optimized for Apple Silicon (M-series / ARM NEON) via Carry-Less Multiplication (`PMULL`), `kjson` achieves over 5.5 GiB/s sustained parsing speeds—over 4x faster than `sonic-rs`.

## Features
- **Zero-Decode Architecture:** Avoids full deserialization until you access a specific field.
- **SIMD Optimized:** Uses architecture-specific intrinsics (like ARM NEON `PMULL`) to track structural tokens.
- **Schema JIT Parsing:** With `kjson-derive`, bind JSON instantly into typed Rust structs at 5+ GiB/s.
- **Hardware-Aware Memory Access:** Pre-fetches byte chunks into L1 cache for zero CPU stalling.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
kjson = "0.2.0"
kjson-derive = "0.2.0"
```

## Quick Start

```rust
use kjson::KowitoView;
use kjson_derive::from_kview;

#[derive(Debug)]
struct User {
    id: i64,
    name: String,
    is_active: bool,
}

from_kview!(User {
    id: i64,
    name: String,
    is_active: bool,
});

fn main() {
    let json_bytes = br#"{"id": 42, "name": "Kowito", "is_active": true}"#;
    
    // Scan and build the structural tape blazing fast
    let view = KowitoView::new(json_bytes);
    
    // Instantly bind to a struct
    let user = User::from_kview(&view).unwrap();
    
    println!("Parsed User: {:?}", user);
}
```

## Under the Hood
Most parsers build an AST or evaluate string quotes using branching logic. `kjson` uses SIMD Carry-Less Multiplication (Polynomial Math) to trace out string blocks parity in a single CPU cycle without branching. This mathematically perfect parsing removes branch mispredictions, maximizing the throughput of modem superscalar processors.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
