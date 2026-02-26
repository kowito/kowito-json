# kowito-json

**A high-performance zero-decode JSON parser.**

`kowito-json` is a highly-optimized JSON parsing and binding library for Rust. Leveraging state-of-the-art SIMD instructions, hardware prefetching, and a "Zero-Decode" philosophy, it parses JSON by completely avoiding scalar loops and allocations until absolutely necessary.

Currently optimized for Apple Silicon (M-series / ARM NEON) via Carry-Less Multiplication (`PMULL`), `kowito-json` is capable of handling over 5.5 GiB/s sustained parsing speeds.

## Features
- **Zero-Decode Architecture:** Avoids full deserialization until you access a specific field.
- **SIMD Optimized:** Uses architecture-specific intrinsics (like ARM NEON `PMULL`) to track structural tokens.
- **Schema JIT Parsing:** With `kowito_json_derive`, bind JSON instantly into typed Rust structs at 6.6 GiB/s.
- **Ultra-Fast JIT Serialization:** Generate JSON from structs at 3.9 - 16 GiB/s using compile-time templates and NEON SIMD escaping.
- **Hardware-Aware Memory Access:** Pre-fetches byte chunks into L1 cache for zero CPU stalling.

## Benchmarks

Parsed on Apple Silicon M4 (NEON PMULL optimized). Measurements taken using `criterion` on a 10MB massive JSON payload.

| Parser | Throughput (GiB/s) | Relative Speed (vs `serde_json`) |
| :--- | :--- | :--- |
| **kowito-json** | **~6.48 GiB/s** | **~27x Faster** |
| `sonic-rs` | ~1.31 GiB/s | ~5.4x Faster |
| `simd-json` | ~0.26 GiB/s | ~1.1x Faster |
| `serde_json` | ~0.24 GiB/s | 1x (Baseline) |

### Serialization Performance (GiB/s)

Measurements taken on Apple Silicon M4.

| Serializer | Tiny (3 fields) | Medium (7 fields) | Numeric (8 fields) |
| :--- | :--- | :--- | :--- |
| **kowito-json (JIT)** | **12.3 ns** | **37.5 ns** | **84.5 ns** |
| `sonic-rs` | 21.3 ns | 64.9 ns | 105.2 ns |
| `serde_json` | 33.7 ns | 83.6 ns | 117.4 ns |

### Mass-String Throughput (GiB/s)
Best for large blobs, logs, or base64 data.

| Serializer | 10KB String (NEON x4) |
| :--- | :--- |
| **kowito-json (JIT)** | **~22.76 GiB/s** |
| `sonic-rs` | ~32.90 GiB/s |
| `serde_json` | ~3.62 GiB/s |

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
kowito-json = "0.2.2"
kowito-json-derive = "0.2.1"
```

## Quick Start

```rust
use kowito_json::{KView, Scratchpad};
use kowito_json::scanner::Scanner;
use kowito_json_derive::KJson;

#[derive(Debug, KJson)]
struct User {
    id: i64,
    name: String,
    active: bool,
    balance: f64,
}

fn main() {
    let json_bytes = br#"{"id": 42, "name": "Kowito", "active": true, "balance": 1234.56}"#;
    
    // 1. Scan and parse
    let mut scratch = Scratchpad::new(1024);
    let tape = scratch.get_mut_tape();
    Scanner::new(json_bytes).scan(tape);
    let view = KView::new(json_bytes, tape);
    let user = User::from_kview(&view); // Ultra-fast zero-decode

    // 2. Serialize at memory-bandwidth speeds
    let mut out_buf = Vec::with_capacity(128);
    user.to_json_bytes(&mut out_buf);
    println!("Serialized: {}", String::from_utf8_lossy(&out_buf));
}
```

## Advanced Examples

### Nested Structs

The `KJson` derive macro automatically generates optimized templates for nested types.

```rust
#[derive(KJson)]
pub struct Address {
    pub city: String,
    pub zip: u32,
}

#[derive(KJson)]
pub struct Profile {
    pub user_id: u64,
    pub address: Address,
    pub tags: Vec<String>, // Coming soon in Phase 4
}
```

### Batch Serialization

For scenarios like high-performance logging or API responses with many items, reuse the same buffer to minimize allocations.

```rust
let items = vec![user1, user2, user3];
let mut buffer = Vec::with_capacity(1024 * 10);

for item in items {
    item.to_json_bytes(&mut buffer);
    buffer.push(b'\n'); // Newline delimited JSON (NDJSON)
}
```

### Custom Serialization

While `#[derive(KJson)]` is recommended for maximum performance, you can manually implement the `Serialize` trait.

```rust
use kowito_json::serialize::{Serialize, write_str_escape, write_value};

struct CustomLog {
    level: String,
    msg: String,
}

impl Serialize for CustomLog {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.push(b'{');
        buf.extend_from_slice(b"\"level\":");
        write_str_escape(buf, self.level.as_bytes());
        buf.push(b',');
        buf.extend_from_slice(b"\"msg\":");
        write_str_escape(buf, self.msg.as_bytes());
        buf.push(b'}');
    }
}
```

## Under the Hood
### Parsing
Most parsers build an AST or evaluate string quotes using branching logic. `kowito-json` uses SIMD Carry-Less Multiplication (Polynomial Math) to trace out string blocks parity in a single CPU cycle without branching. This mathematically perfect parsing removes branch mispredictions, maximizing the throughput of modern superscalar processors.

### Serialization
`kowito-json` uses **Schema-JIT Serialization**. Instead of using generic reflection or slow `std::fmt` traits, the `KJson` macro generates a specialized `to_json_bytes` method at compile-time. This method:
- Interleaves field keys and structural characters as static byte slices (`memcpy` from RO data).
- Uses `itoa` and `ryu` for branchless numeric formatting.
- Employs **ARM NEON SIMD** processing to scan 16-byte blocks for escape characters in a single cycle.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
