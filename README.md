# kowito-json

**A high-performance zero-decode JSON parser and schema-JIT serializer for Rust.**

`kowito-json` parses and serializes JSON at memory-bandwidth speeds using ARM NEON Carry-Less Multiplication (`PMULL`), zero-copy tape scanning, and compile-time schema baking via the `#[derive(KJson)]` macro.

> Optimized for Apple Silicon (M-series / aarch64). AVX2 and portable-SIMD paths available.

## Features

- **Zero-Decode Parser** — scans JSON into a flat `u32` tape without allocating or decoding; fields are read lazily on access.
- **SIMD String Tracking** — uses `PMULL` carry-less multiplication to compute string-mask parity across 16-byte chunks in a single cycle, eliminating all branch mispredictions.
- **Schema-JIT Serializer** — `#[derive(KJson)]` bakes field key prefixes as `&'static [u8]` at compile time; the hot path is pure `memcpy` + `itoa`/`ryu`.
- **NEON SIMD Escape Scanning** — string escaping scans 16 bytes per cycle; only slows for rare escape characters.
- **Hardware Prefetch** — `std::intrinsics::prefetch_read_data` keeps the next chunk in L1 while the current one is processed.
- **Arena Allocator** — `Scratchpad` and thread-local `with_scratch_tape` eliminate per-parse heap allocation.

## Benchmarks

Measured on **Apple Silicon M4**, release profile, using `criterion` (100 samples, 95% CI).

### Parsing — 10 MB Massive JSON Array

**Visual Chart (Higher = Faster)**

```
kowito-json ████████████████████████████ 6.48 GiB/s ⭐ FASTEST
sonic_rs    ████ 1.31 GiB/s
simd_json   ░ 0.26 GiB/s
serde_json  ░ 0.24 GiB/s (baseline)
```

| Parser | Throughput | vs `serde_json` |
|:---|:---|:---|
| **kowito-json** | **~6.48 GiB/s** | **27× faster** |
| `sonic-rs` | ~1.31 GiB/s | 5.4× faster |
| `simd-json` | ~0.26 GiB/s | 1.1× faster |
| `serde_json` | ~0.24 GiB/s | baseline |

---

### Serialization — Micro Payloads (Lower Latency = Better)

**Tiny (3 fields)**
```
serde_json  ████████████████████████ 32.5 ns
sonic_rs    ████████████████ 21.5 ns
kowito-json ███ 9.88 ns ⭐ FASTEST (3.3× faster)
```

**Medium (7 fields)**
```
serde_json  ████████████████████ 79.3 ns
sonic_rs    ████████████ 63.2 ns
kowito-json ███████ 33.8 ns ⭐ FASTEST (2.3× faster)
```

**Numeric (8 fields)**
```
serde_json  ███████████████ 114.4 ns
sonic_rs    ████████████ 99.0 ns
kowito-json ██████████ 79.8 ns ⭐ FASTEST (1.4× faster)
```

| Payload | `serde_json` | `sonic_rs` | **kowito-json** | Gain |
|:---|:---|:---|:---|:---|
| Tiny — 3 fields | 32.5 ns | 21.5 ns | **9.88 ns** | **3.3×** |
| Medium — 7 fields | 79.3 ns | 63.2 ns | **33.8 ns** | **2.3×** |
| Numeric — 8 fields | 114.4 ns | 99.0 ns | **79.8 ns** | **1.4×** |

---

### Serialization — Hot Loop (1 000 items)

**Latency per Batch**
```
serde_json  ███████████████████████████████ 87.1 µs
sonic_rs    █████████████████ 70.1 µs
kowito-json ████████ 39.6 µs ⭐ FASTEST (2.2× faster)
```

**Throughput**
```
serde_json  ███ 1.25 GiB/s
sonic_rs    ████ 1.55 GiB/s
kowito-json ███████ 2.75 GiB/s ⭐ FASTEST
```

| Serializer | Latency | Throughput |
|:---|:---|:---|
| **kowito-json** | **39.6 µs** | **2.75 GiB/s** |
| `sonic_rs` | 70.1 µs | 1.55 GiB/s |
| `serde_json` | 87.1 µs | 1.25 GiB/s |

---

### Serialization — Large String (10 KB, SIMD fast-path)

**Latency (Lower = Better)**
```
sonic_rs    █ 281 ns ⭐ FASTEST
kowito-json ██ 370 ns (competitive)
serde_json  ████████████████ 2542 ns (9× slower)
```

**Throughput (Higher = Better)**
```
sonic_rs    ████████████████████████████ 33.2 GiB/s ⭐ FASTEST
kowito-json ████████████████████ 25.0 GiB/s
serde_json  ████ 3.66 GiB/s
```

| Serializer | Latency | Throughput |
|:---|:---|:---|
| `sonic_rs` | **281 ns** | **33.2 GiB/s** |
| **kowito-json** | 370 ns | 25.0 GiB/s |
| `serde_json` | 2542 ns | 3.66 GiB/s |

---

### 📊 Summary: When to Use Each

| Use Case | Best Choice | Why |
|:---|:---|:---|
| **Micro payloads** (< 100 bytes) | **kowito-json** ⭐ | 3.3× speedup, zero-copy design |
| **Hot-loop batch** (1000+ items) | **kowito-json** ⭐ | 2.2× faster, schema-JIT wins |
| **Large strings** (10KB+) | `sonic_rs` | Specialized escape SIMD, 33 GiB/s |
| **General parsing** (all sizes) | **kowito-json** ⭐ | 27× faster than serde_json |
| **Compatibility** (stable Rust) | `serde_json` | Mature, works on stable |

> **kowito-json dominates micro and hot-loop workloads.** sonic_rs edges ahead only on pure large-string throughput. Choose **kowito-json** for microservices, logging pipelines, and real-time systems.

---

## Feature Comparison

| Feature | kowito-json | sonic_rs | serde_json |
|:---|:---:|:---:|:---:|
| **Parsing Speed** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| **Serialization** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Zero-Decode** | ✅ | ❌ | ❌ |
| **Schema-JIT** | ✅ | ❌ | ❌ |
| **SIMD String Escape** | ✅ NEON | ✅ AVX2/SSE | ❌ |
| **Arena Allocator** | ✅ | ❌ | ❌ |
| **Stable Rust** | ❌ (nightly) | ✅ | ✅ |
| **Cross-Platform** | ARM NEON | AVX2/portable | ✅ Universal |

## Installation

```toml
[dependencies]
kowito-json        = "0.2.5"
kowito-json-derive = "0.2.3"
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
