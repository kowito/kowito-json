# kowito-json

**A high-performance zero-decode JSON parser and schema-JIT serializer for Rust.**

`kowito-json` parses and serializes JSON at memory-bandwidth speeds using ARM NEON Carry-Less Multiplication (`PMULL`), x86_64 AVX2+PCLMULQDQ, zero-copy tape scanning, and compile-time schema baking via the `#[derive(KJson)]` macro.

> Optimized for Apple Silicon (M-series / aarch64) and x86_64 AVX2. Zero-allocation data pipeline.

## Features

- **Zero-Decode Parser** — scans JSON into a flat `u32` tape without allocating or decoding; fields are read lazily on access.
- **SIMD String Tracking** — uses `PMULL` carry-less multiplication to compute string-mask parity across 16-byte chunks in a single cycle, eliminating all branch mispredictions.
- **Schema-JIT Serializer** — `#[derive(KJson)]` bakes field key prefixes as `&'static [u8]` at compile time; the hot path is pure `memcpy` + `itoa`/`ryu`.
- **NEON SIMD Escape Scanning** — string escaping scans 16 bytes per cycle; only slows for rare escape characters.
- **Hardware Prefetch** — `std::intrinsics::prefetch_read_data` keeps the next chunk in L1 while the current one is processed.
- **Arena Allocator** — `Scratchpad` and thread-local `with_scratch_tape` eliminate per-parse heap allocation.

## Benchmarks

Measured on **Apple Silicon M4**, release profile, using `criterion` (100 samples, 95% CI).  
**Note:** x86_64 AVX2+PCLMULQDQ path is fully implemented and provides consistent high throughput on Intel/AMD platforms.

### Parsing — 12 MB Real-World JSON Corpus (100k user objects)

**Visual Chart (Higher = Faster)**

```
kowito-json ████████████████████████████████ 5.30 GiB/s ⭐ FASTEST
sonic_rs    ████████ 1.17 GiB/s
simd_json   █ 0.234 GiB/s
serde_json  █ 0.211 GiB/s (baseline)
```

| Parser | Throughput | vs `serde_json` |
|:---|:---|:---|
| **kowito-json** | **~5.30 GiB/s** | **25× faster** |
| `sonic-rs` | ~1.17 GiB/s | 5.5× faster |
| `simd-json` | ~0.234 GiB/s | 1.1× faster |
| `serde_json` | ~0.211 GiB/s | baseline |

---

### Serialization — Micro Payloads (Lower Latency = Better)

**Tiny (3 fields)**
```
serde_json  ████████████████████████████ 43.2 ns
sonic_rs    ███████████████ 23.4 ns
kowito-json ████████ 12.9 ns ⭐ FASTEST (3.3× faster)
```

**Medium (7 fields)**
```
serde_json  ████████████████████████████ 101.7 ns
sonic_rs    ██████████████████ 67.0 ns
kowito-json ████████████ 41.7 ns ⭐ FASTEST (2.4× faster)
```

**Numeric (8 fields)**
```
serde_json  ████████████████████████████ 140.1 ns
sonic_rs    ████████████████████████ 118.1 ns
kowito-json ███████████████████ 92.8 ns ⭐ FASTEST (1.5× faster)
```

| Payload | `serde_json` | `sonic_rs` | **kowito-json** | Gain |
|:---|:---|:---|:---|:---|
| Tiny — 3 fields | 43.2 ns | 23.4 ns | **12.9 ns** | **3.3×** |
| Medium — 7 fields | 101.7 ns | 67.0 ns | **41.7 ns** | **2.4×** |
| Numeric — 8 fields | 140.1 ns | 118.1 ns | **92.8 ns** | **1.5×** |

---

### Serialization — Hot Loop (1 000 items)

**Latency per Batch**
```
serde_json  ████████████████████████████ 114.4 µs
sonic_rs    ████████████████████ 80.0 µs
kowito-json ███████████████ 60.0 µs ⭐ FASTEST (1.9× faster)
```

**Throughput**
```
kowito-json ████████████████████████████ 1.82 GiB/s ⭐ FASTEST
sonic_rs    █████████████████████ 1.36 GiB/s
serde_json  ███████████████ 0.95 GiB/s
```

| Serializer | Latency | Throughput |
|:---|:---|:---|
| **kowito-json** | **60.0 µs** | **1.82 GiB/s** |
| `sonic_rs` | 80.0 µs | 1.36 GiB/s |
| `serde_json` | 114.4 µs | 0.95 GiB/s |

---

### Serialization — Large String (10 KB, SIMD fast-path)

**Latency (Lower = Better)**
```
kowito-json ███ 308.1 ns ⭐ FASTEST
sonic_rs    ████ 320.0 ns
serde_json  ████████████████████████████ 3291 ns (10.7× slower)
```

**Throughput (Higher = Better)**
```
kowito-json ████████████████████████████ 30.3 GiB/s ⭐ FASTEST
sonic_rs    ███████████████████████████ 29.2 GiB/s
serde_json  ███ 2.84 GiB/s
```

| Serializer | Latency | Throughput |
|:---|:---|:---|
| **kowito-json** | **308.1 ns** | **30.3 GiB/s** |
| `sonic_rs` | 320.0 ns | 29.2 GiB/s |
| `serde_json` | 3291 ns | 2.84 GiB/s |

---

### 📊 Summary: When to Use Each

| Use Case | Best Choice | Why |
|:---|:---|:---|
| **Micro payloads** (< 100 bytes) | **kowito-json** ⭐ | 3.3× speedup, zero-copy design |
| **Hot-loop batch** (1000+ items) | **kowito-json** ⭐ | 1.9× faster, schema-JIT wins |
| **Large strings** (10KB+) | **kowito-json** ⭐ | 30.3 GiB/s, single-pass NEON scan+store |
| **General parsing** (all sizes) | **kowito-json** ⭐ | 25× faster than serde_json |
| **Compatibility** (stable Rust) | `serde_json` | Mature, works on stable |

> **kowito-json is fastest across all workloads** — micro payloads, hot-loop batch, large-string throughput, and parsing. Choose **kowito-json** for microservices, logging pipelines, and real-time systems.

---

## Feature Comparison

| Feature | kowito-json | sonic_rs | serde_json |
|:---|:---:|:---:|:---:|
| **Parsing Speed** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| **Serialization** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Zero-Decode** | ✅ | ❌ | ❌ |
| **Schema-JIT** | ✅ | ❌ | ❌ |
| **SIMD String Escape** | ✅ NEON / AVX2 | ✅ AVX2/SSE | ❌ |
| **Arena Allocator** | ✅ | ❌ | ❌ |
| **Stable Rust** | ❌ (nightly) | ✅ | ✅ |
| **Architecture** | ARM NEON / AVX2 | AVX2 / SSE | ✅ Universal |

## Architecture Support

- **ARM64 (Apple Silicon / Graviton)**: Uses `PMULL` (Carry-less multiplication) for string detection and NEON for structural scanning.
- **x86_64 (Intel Core / AMD Ryzen)**: Uses `AVX2` and `PCLMULQDQ` for high-speed scanning.
- **Experimental (M4+)**: Prototypes for `SVE2` (via `svmatch`) and `AMX` (Whitespace Scrubber) are in development.

## Installation

```toml
[dependencies]
kowito-json        = "0.2.12"
kowito-json-derive = "0.2.12"
```

Requires **Rust nightly** (uses `portable_simd`):

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly"
```


## Quick Start

### Serialization

```rust
use kowito_json_derive::KJson;

#[derive(Debug, KJson)]
struct User {
    id: u64,
    name: String,
    active: bool,
    score: f64,
}

fn main() {
    let user = User { id: 1, name: "Alice".to_string(), active: true, score: 98.6 };

    let mut buf = Vec::new();
    user.to_json_bytes(&mut buf);

    println!("{}", std::str::from_utf8(&buf).unwrap());
    // {"id":1,"name":"Alice","active":true,"score":98.6}
}
```

### Parsing (Zero-Decode)

```rust
use kowito_json::{KView, Scratchpad};
use kowito_json::scanner::Scanner;
use kowito_json_derive::KJson;

#[derive(Debug, KJson)]
struct User {
    id: i64,
    name: String,
    active: bool,
}

fn main() {
    let json = br#"{"id": 42, "name": "Kowito", "active": true}"#;

    let mut scratch = Scratchpad::new(1024);
    let tape = scratch.get_mut_tape();

    // SIMD scan — fills tape with structural token offsets
    let n = Scanner::new(json).scan(tape);

    // Zero-copy view — no allocation, no string decoding
    let view = KView::new(json, &tape[..n]);
    let user = User::from_kview(&view);

    println!("{user:?}");
}
```

## Examples

Run any example with `cargo run --example <name>`.

### All examples

| Example | Command | What it shows |
|:---|:---|:---|
| Basic serialization | `cargo run --example 01_basic_serialize` | `#[derive(KJson)]`, `to_json_bytes()` |
| All primitive types | `cargo run --example 02_all_types` | integers, floats, bools, all string escapes |
| Advanced types | `cargo run --example 03_advanced_types` | `Option`, `Vec`, `Box`, `Cow`, nested structs |
| Arena allocator | `cargo run --example 04_arena_scratch` | `Scratchpad`, `with_scratch_tape`, reuse patterns |
| Low-level scanner | `cargo run --example 05_scanner` | `Scanner::scan`, tape inspection |
| Hot-loop batch | `cargo run --example 06_hot_loop` | NDJSON stream, JSON array, server buffer reuse |
| Manual `Serialize` | `cargo run --example 07_manual_serialize` | renamed fields, skip-null, tagged enum |
| SIMD string writer | `cargo run --example 08_string_escape` | `write_str_escape` directly, control chars |

### Batch serialization (NDJSON)

```rust
use kowito_json_derive::KJson;

#[derive(KJson)]
struct LogEntry { timestamp: u64, level: String, message: String }

let entries = vec![
    LogEntry { timestamp: 1_700_000_001, level: "INFO".into(), message: "started".into() },
    LogEntry { timestamp: 1_700_000_002, level: "WARN".into(), message: "slow query".into() },
];

let mut buf = Vec::with_capacity(entries.len() * 128);
for entry in &entries {
    entry.to_json_bytes(&mut buf);
    buf.push(b'\n');
}
println!("{}", std::str::from_utf8(&buf).unwrap());
```

### Arena-backed parsing (zero allocation)

```rust
use kowito_json::arena::with_scratch_tape;
use kowito_json::scanner::Scanner;

let jsons: &[&[u8]] = &[
    br#"{"id":1,"val":"alpha"}"#,
    br#"{"id":2,"val":"beta"}"#,
];

for json in jsons {
    with_scratch_tape(|tape| {
        let n = Scanner::new(json).scan(tape);
        println!("{} tokens", n);
    });
}
```

### Manual `Serialize` implementation

```rust
use kowito_json::serialize::{Serialize, write_str_escape, write_value};

struct ApiResponse {
    status: u32,
    data: Option<String>,
}

impl Serialize for ApiResponse {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(b"{\"status\":");
        write_value(&self.status, buf);
        if let Some(ref d) = self.data {
            buf.extend_from_slice(b",\"data\":");
            write_str_escape(buf, d.as_bytes());
        }
        buf.push(b'}');
    }
}
```

### Nested structs

```rust
use kowito_json_derive::KJson;

#[derive(KJson)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip: String,
}

#[derive(KJson)]
pub struct Company {
    pub name: String,
    pub employee_count: u32,
    pub hq: Address,
}
```

> Nested `KJson` structs serialize correctly because each implements `SerializeRaw` — the outer struct's JIT template calls the inner one directly without boxing.

## Under the Hood

### Parsing — SIMD String Parity via PMULL

Traditional parsers scan for `"` with scalar loops. `kowito-json` instead computes the **string block mask** using ARM NEON `vmull_p64` (carry-less multiply):

```
quote_mask = PMULL(quote_positions, 0xFFFF…)  // XOR-prefix-sum in one instruction
string_mask = quote_mask XOR prev_in_string    // carry across 64-byte blocks
```

This gives a bitmask where every bit inside a string is 1, outside is 0 — enabling branchless structural token extraction. The result is a flat `u32` tape of byte offsets; no AST, no allocation.

### Serialization — Schema-JIT Templates

`#[derive(KJson)]` runs at compile time and emits code equivalent to:

```rust
// Generated (simplified):
pub fn to_json_bytes(&self, buf: &mut Vec<u8>) {
    buf.reserve(STATIC_CAP + dynamic_cap);  // single pre-allocation
    unsafe {
        let mut p = buf.as_mut_ptr().add(buf.len());
        copy_nonoverlapping(b"{\"id\":".as_ptr(), p, 6);  p = p.add(6);
        p = itoa_raw(self.id, p);
        copy_nonoverlapping(b",\"name\":\"".as_ptr(), p, 9);  p = p.add(9);
        p = neon_escape_str(self.name.as_bytes(), p);
        // ... remaining fields ...
        *p = b'}'; p = p.add(1);
        buf.set_len(p.offset_from(buf.as_ptr()) as usize);
    }
}
```

All field key bytes live in the read-only data segment. The hot path is a straight-line sequence of `memcpy` + numeric writes + SIMD escape — no branches, no reflection.

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
