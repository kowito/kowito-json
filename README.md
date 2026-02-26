# kowito-json

**A high-performance zero-decode JSON parser.**

`kowito-json` is a highly-optimized JSON parsing and binding library for Rust. Leveraging state-of-the-art SIMD instructions, hardware prefetching, and a "Zero-Decode" philosophy, it parses JSON by completely avoiding scalar loops and allocations until absolutely necessary.

Currently optimized for Apple Silicon (M-series / ARM NEON) via Carry-Less Multiplication (`PMULL`), `kowito-json` is capable of handling over 5.5 GiB/s sustained parsing speeds.

## Features
- **Zero-Decode Architecture:** Avoids full deserialization until you access a specific field.
- **SIMD Optimized:** Uses architecture-specific intrinsics (like ARM NEON `PMULL`) to track structural tokens.
- **Schema JIT Parsing:** With `kowito-json-derive`, bind JSON instantly into typed Rust structs at 6,600+ MiB/s.
- **Hardware-Aware Memory Access:** Pre-fetches byte chunks into L1 cache for zero CPU stalling.

## Benchmarks

Parsed on Apple Silicon M4 (NEON PMULL optimized). Measurements taken using `criterion` on a 10MB massive JSON payload.

| Parser | Throughput (MiB/s) | Relative Speed (vs `serde_json`) |
| :--- | :--- | :--- |
| **kowito-json** | **~6,635 MiB/s** | **~27x Faster** |
| `sonic-rs` | ~1,341 MiB/s | ~5.4x Faster |
| `simd-json` | ~276 MiB/s | ~1.1x Faster |
| `serde_json` | ~245 MiB/s | 1x (Baseline) |

### Performance Visualization (MiB/s)

```text
serde_json  [■] 245
simd-json   [■] 276
sonic-rs    [■■■■■] 1341
kowito-json [■■■■■■■■■■■■■■■■■■■■■■■■■■■] 6635
```

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
kowito-json = "0.2.0"
kowito-json-derive = "0.2.0"
```

## Quick Start

```rust
use kowito_json::KView;
use kowito_json_derive::from_kview;

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
    let view = KView::new(json_bytes);
    
    // Instantly bind to a struct
    let user = User::from_kview(&view).unwrap();
    
    println!("Parsed User: {:?}", user);
}
```

## Under the Hood
Most parsers build an AST or evaluate string quotes using branching logic. `kowito-json` uses SIMD Carry-Less Multiplication (Polynomial Math) to trace out string blocks parity in a single CPU cycle without branching. This mathematically perfect parsing removes branch mispredictions, maximizing the throughput of modem superscalar processors.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
