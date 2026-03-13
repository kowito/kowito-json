# Kowito-JSON Benchmark Report

**Date:** March 13, 2026  
**Version:** 0.2.15  
**Compiler:** Rust (2026-03-12)

---

## 📊 Executive Summary

Kowito-JSON v0.2.15 establishes a **new performance record**. By combining a **branchless bit-packed tape format** with **merged SIMD character class detection**, we've achieved a massive throughput increase across all parsing and serialization workloads.

The NEON scanner now reaches **7.98 GiB/s** on a real-world JSON corpus — **6.2× faster** than `sonic_rs` and **33×** faster than `serde_json`.

| Category | Winner | Speedup vs serde_json | Speedup vs sonic_rs |
|----------|--------|----------------------|---------------------|
| Tiny Payloads (3 fields) | **Kowito** | **2.9x** | **1.8x** |
| Medium Payloads (7 fields) | **Kowito** | **2.2x** | **1.7x** |
| Numeric Payloads (8 fields) | **Kowito** | **1.4x** | **1.3x** |
| Hot Loop (1000 items) | **Kowito** | **1.9x** | **1.5x** |
| Massive Parser – scanner only | **Kowito** | **33x** | **6.2x** |
| Massive Parser – schema JIT | **Kowito** | **32x** | **6.1x** |

---

## 1️⃣ Micro-Payload Serialization: tiny_3fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.47 GiB/s
sonic_rs        ███████ 2.34 GiB/s
kowito_json_jit █████████████ 4.27 GiB/s ⭐ FASTEST
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      ████████████████ 34.2 ns
sonic_rs        ██████████ 21.6 ns
kowito_json_jit █████ 11.9 ns ⭐ FASTEST
```

---

## 2️⃣ Micro-Payload Serialization: medium_7fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.36 GiB/s
sonic_rs        ██████ 1.83 GiB/s
kowito_json_jit ███████████ 3.09 GiB/s ⭐ FASTEST
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      █████████████████████ 84.4 ns
sonic_rs        ████████████████ 62.9 ns
kowito_json_jit ██████████ 37.5 ns ⭐ FASTEST
```

---

## 3️⃣ Micro-Payload Serialization: numeric_8fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.32 GiB/s
sonic_rs        ████ 1.41 GiB/s
kowito_json_jit ██████ 1.89 GiB/s ⭐ FASTEST
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      ███████████████ 108.5 ns
sonic_rs        ██████████████ 101.5 ns
kowito_json_jit ███████████ 77.2 ns ⭐ FASTEST
```

---

## 4️⃣ Hot-Loop Serialization (1,000 items)

**Throughput Comparison (Higher is Better)**

```
serde_json      ████████████████ 1.15 GiB/s
sonic_rs        ██████████████████████ 1.54 GiB/s
kowito_json_jit ████████████████████████████████ 2.29 GiB/s ⭐ FASTEST
```

**Latency (Microseconds - Lower is Better)**

```
serde_json      ███████████████████ 94.5 µs
sonic_rs        ██████████████ 70.6 µs
kowito_json_jit ██████████ 47.6 µs ⭐ FASTEST
```

---

## 5️⃣ Large-String Serialization (SIMD) - 10KB Strings

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 3.57 GiB/s
sonic_rs        ████████████████████████████████ 32.4 GiB/s ⭐ FASTEST
kowito_json_jit ███████████████████████ 23.1 GiB/s
```

**Analysis:** Competitive standing at **23.1 GiB/s**.

---

## 6️⃣ Massive JSON Parser – 12 MB Real-World Corpus (NEON baseline, aarch64)

**Scanner Throughput (Higher is Better)**

```
serde_json              ▌ 241 MiB/s
simd_json               ▌ 271 MiB/s
sonic_rs                █████ 1.28 GiB/s
kowito_scanner_only     ██████████████████████████████████████████ 7.98 GiB/s ⭐
kowito_schema_jit       █████████████████████████████████████████ 7.76 GiB/s ⭐
```

**Latency (per 12 MB parse, Lower is Better)**

```
serde_json              ████████████████████████████████ 51.5 ms
simd_json               ████████████████████████████ 48.7 ms
sonic_rs                ██████ 10.1 ms
kowito_scanner_only     ▌ 1.62 ms ⭐ FASTEST
kowito_schema_jit       ▌ 1.66 ms ⭐ FASTEST
```

| Library | Time (ms) | Throughput | vs serde_json | vs sonic_rs |
|---|---|---|---|---|
| serde_json | 51.5 | 241 MiB/s | — | — |
| simd_json | 48.7 | 271 MiB/s | 1.1× | — |
| sonic_rs | 10.1 | 1.28 GiB/s | 5.3× | — |
| **kowito_scanner_only** | **1.62** | **7.98 GiB/s** | **33.0×** | **6.2×** |
| **kowito_schema_jit** | **1.66** | **7.76 GiB/s** | **32.1×** | **6.1×** |

---

## 🎯 Technical Breakthroughs in v0.2.15

1. **Branchless Tape Tagging**: Replaced `match` logic with a static `TAG_TABLE` lookup, eliminating all branch mispredictions during tape generation.
2. **SIMD Character Merging**: Used bit-5 ORing (`v | 32`) to collapse `{/[` and `}/]` character classes, reducing NEON/AVX2 structural search from 6 comparisons to 4.
3. **Hoisted Constants**: Fixed a regression in v0.2.14 by hoisting the SIMD bitmask load out of the 64-byte hot loop.
