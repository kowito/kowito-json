# kowito-json

**A high-performance zero-decode JSON parser.**

`kowito-json` is a highly-optimized JSON parsing and binding library for Rust. Leveraging state-of-the-art SIMD instructions, hardware prefetching, and a "Zero-Decode" philosophy, it parses JSON by completely avoiding scalar loops and allocations until absolutely necessary.

Currently optimized for Apple Silicon (M-series / ARM NEON) via Carry-Less Multiplication (`PMULL`), `kowito-json` is capable of handling over 5.5 GiB/s sustained parsing speeds.

## Features
- **Zero-Decode Architecture:** Avoids full deserialization until you access a specific field.
- **SIMD Optimized:** Uses architecture-specific intrinsics (like ARM NEON `PMULL`) to track structural tokens.
- **Schema JIT Parsing:** With `kowito_json_derive`, bind JSON instantly into typed Rust structs at 6,600+ MiB/s.
- **Ultra-Fast Schema-JIT Serialization:** Generate JSON from structs at 4,300+ MiB/s using compile-time templates.
- **Hardware-Aware Memory Access:** Pre-fetches byte chunks into L1 cache for zero CPU stalling.

## Benchmarks

Parsed on Apple Silicon M4 (NEON PMULL optimized). Measurements taken using `criterion` on a 10MB massive JSON payload.

| Parser | Throughput (MiB/s) | Relative Speed (vs `serde_json`) |
| :--- | :--- | :--- |
| **kowito-json** | **~6,635 MiB/s** | **~27x Faster** |
| `sonic-rs` | ~1,341 MiB/s | ~5.4x Faster |
| `simd-json` | ~276 MiB/s | ~1.1x Faster |
| `serde_json` | ~245 MiB/s | 1x (Baseline) |

### Parsing Performance (MiB/s)

```text
serde_json  [■] 245
simd-json   [■] 276
sonic-rs    [■■■■■] 1341
kowito-json [■■■■■■■■■■■■■■■■■■■■■■■■■■■] 6635
```

### Serialization Performance (MiB/s)

Measurements taken on small payloads (3-8 fields).

| Serializer | Throughput (MiB/s) | Relative Speed (vs `serde_json`) |
| :--- | :--- | :--- |
| **kowito-json (JIT)** | **~4,350 MiB/s** | **~2.8x Faster** |
| `sonic-rs` | ~1,750 MiB/s | ~1.1x Faster |
| `serde_json` | ~1,520 MiB/s | 1x (Baseline) |

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
kowito-json = "0.2.0"
kowito-json-derive = "0.2.0"
```

## Quick Start

```rust
use kowito_json::{KView, Scratchpad};
use kowito_json::scanner::Scanner;
use kowito_json_derive::Kjson;

#[derive(Debug, Kjson)]
struct User {
    id: i64,
    name: String,
    is_active: bool,
}

fn main() {
    let json_bytes = br#"{"id": 42, "name": "Kowito", "is_active": true}"#;
    
    // 1. Allocate a scratchpad for the tape (usually kept thread-local)
    let mut scratchpad = Scratchpad::new(1024);
    let tape = scratchpad.get_mut_tape();
    
    // 2. Scan and find all structural characters instantly with SIMD
    let scanner = Scanner::new(json_bytes);
    scanner.scan(tape);
    
    // 3. Create a zero-decode view
    let view = KView::new(json_bytes, tape);
    
    // 4. Instantly bind to a struct
    let user = User::from_kview(&view).unwrap();
    println!("Parsed User: {:?}", user);

    // 5. Serialize back to JSON at memory-bandwidth speeds
    let mut out_buf = Vec::new();
    user.to_kbytes(&mut out_buf);
    println!("Serialized: {}", String::from_utf8(out_buf).unwrap());
}
```

## Under the Hood
### Parsing
Most parsers build an AST or evaluate string quotes using branching logic. `kowito-json` uses SIMD Carry-Less Multiplication (Polynomial Math) to trace out string blocks parity in a single CPU cycle without branching. This mathematically perfect parsing removes branch mispredictions, maximizing the throughput of modern superscalar processors.

### Serialization
`kowito-json` uses **Schema-JIT Serialization**. Instead of using generic reflection or slow `std::fmt` traits, the `Kjson` macro generates a specialized `to_kbytes` method at compile-time. This method:
- Interleaves field keys and structural characters as static byte slices (`memcpy` from RO data).
- Uses `itoa` and `ryu` for branchless numeric formatting.
- Employs a specialized lookup-table for escape-fast-path string processing.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
