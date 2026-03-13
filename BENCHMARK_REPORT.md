# Kowito-JSON Benchmark Report

**Date:** March 13, 2026  
**Version:** 0.2.14  
**Compiler:** Rust (2026-03-12)

---

## 📊 Executive Summary

Kowito-JSON continues to demonstrate **superior performance**. The latest version (0.2.14) introduces a **bit-packed tape format** and **loop unrolling**, maintaining our lead in parsing and serialization. The NEON scanner reaches **2.5 GiB/s** on a 12 MB real-world JSON corpus — **2× faster** than sonic_rs and **10×** faster than serde_json.

| Category | Winner | Speedup vs serde_json | Speedup vs sonic_rs |
|----------|--------|----------------------|---------------------|
| Tiny Payloads (3 fields) | **Kowito** | **2.6x** | **1.7x** |
| Medium Payloads (7 fields) | **Kowito** | **2.2x** | **1.7x** |
| Numeric Payloads (8 fields) | **Kowito** | **1.4x** | **1.3x** |
| Hot Loop (1000 items) | **Kowito** | **2.0x** | **1.6x** |
| Massive Parser – scanner only | **Kowito** | **10x** | **2.0x** |
| Massive Parser – schema JIT | **Kowito** | **11x** | **2.1x** |

---

## 1️⃣ Micro-Payload Serialization: tiny_3fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.43 GiB/s
sonic_rs        ███████ 2.27 GiB/s
kowito_json_jit ████████████ 3.93 GiB/s ⭐ FASTEST
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      ████████████████ 34.2 ns
sonic_rs        ██████████ 21.6 ns
kowito_json_jit ██████ 13.0 ns ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **2.6x faster** than serde_json, **1.7x faster** than sonic_rs

---

## 2️⃣ Micro-Payload Serialization: medium_7fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.36 GiB/s
sonic_rs        ██████ 1.83 GiB/s
kowito_json_jit ██████████ 3.06 GiB/s ⭐ FASTEST
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      █████████████████████ 84.4 ns
sonic_rs        ████████████████ 62.9 ns
kowito_json_jit █████████ 37.8 ns ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **2.2x faster** than serde_json, **1.7x faster** than sonic_rs

---

## 3️⃣ Micro-Payload Serialization: numeric_8fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.32 GiB/s
sonic_rs        ████ 1.38 GiB/s
kowito_json_jit ██████ 1.87 GiB/s ⭐ FASTEST
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      ███████████████ 108.5 ns
sonic_rs        ██████████████ 103.9 ns
kowito_json_jit ███████████ 77.7 ns ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **1.4x faster** than serde_json, **1.3x faster** than sonic_rs

---

## 4️⃣ Hot-Loop Serialization (1,000 items)

**Throughput Comparison (Higher is Better)**

```
serde_json      ████████████████ 1.15 GiB/s
sonic_rs        ██████████████████████ 1.54 GiB/s
kowito_json_jit ████████████████████████████████ 2.28 GiB/s ⭐ FASTEST
```

**Latency (Microseconds - Lower is Better)**

```
serde_json      ███████████████████ 94.5 µs
sonic_rs        ██████████████ 70.8 µs
kowito_json_jit ██████████ 47.9 µs ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **2.0x faster** than serde_json, **1.5x faster** than sonic_rs

---

## 5️⃣ Large-String Serialization (SIMD) - 10KB Strings

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 3.42 GiB/s
sonic_rs        ████████████████████████████████ 32.8 GiB/s ⭐ FASTEST
kowito_json_jit ███████████████████████ 23.2 GiB/s
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      █████████████████████ 2731 ns
sonic_rs        ▌ 284.6 ns ⭐ FASTEST
kowito_json_jit ███ 401.9 ns
```

**Analysis:** For large strings, sonic_rs' specialized SIMD escape handling remains the gold standard. Kowito remains highly competitive at **23.2 GiB/s**.

---

## 6️⃣ Massive JSON Parser – 12 MB Real-World Corpus (NEON baseline, aarch64)

> **Input:** 100,000 user-object JSON array, ~12.3 MiB  
> **Path:** NEON (ARM)  
> **Note:** Performance numbers in this section were measured on a shared CI environment and may vary slightly from local dev runs.

**Scanner Throughput (Higher is Better)**

```
serde_json              ▌ 241 MiB/s
simd_json               ▌ 268 MiB/s
sonic_rs                ████████ 1.22 GiB/s
kowito_scanner_only     ████████████████████████████████ 2.54 GiB/s ⭐ FASTEST
kowito_schema_jit       █████████████████████████████████ 2.61 GiB/s ⭐ FASTEST
```

**Latency (per 12 MB parse, Lower is Better)**

```
serde_json              ████████████████████████████████ 52.4 ms
simd_json               ████████████████████████████ 49.3 ms
sonic_rs                ██████ 10.6 ms
kowito_scanner_only     █ 5.08 ms ⭐ FASTEST
kowito_schema_jit       █ 4.95 ms ⭐ FASTEST
```

| Library | Time (ms) | Throughput | vs serde_json | vs sonic_rs |
|---|---|---|---|---|
| serde_json | 52.4 | 241 MiB/s | — | — |
| simd_json | 49.3 | 268 MiB/s | 1.1× | — |
| sonic_rs | 10.6 | 1.22 GiB/s | 5.0× | — |
| **kowito_scanner_only** | **5.08** | **2.54 GiB/s** | **10.3×** | **2.1×** |
| **kowito_schema_jit** | **4.95** | **2.61 GiB/s** | **10.6×** | **2.1×** |

**Performance Gain:** ✅ Kowito scanner is **10× faster** than serde_json,  
**2.1× faster** than sonic_rs.

---

## 📉 Performance Categories

### 🚀 Kowito Dominates
- **Tiny payloads** (< 50 bytes): 2.6x speedup
- **Medium payloads** (50-200 bytes): 2.2x speedup  
- **Repeated serialization** (1000+ items): 2.0x speedup
- **Structural Scanning**: 2.1x speedup over next best (sonic_rs)

---

## 🎯 Key Insights

### Why version 0.2.14 is better
1. **Bit-Packed Tape** - Reduces memory traffic by embedding token types directly in the offset stream.
2. **Loop Unrolling** - Reduces branch misses in the escape-scanning hot path.
3. **Clean-Run Optimization** - Accelerated `KString` unescaping via bulk copies.

---

## 📋 Benchmark Methodology

- **Samples:** 100 measurements per benchmark
- **Confidence:** 95% (statistical significance: p < 0.05)
- **Profile:** `bench` (release optimizations enabled)

---

## 🔧 Compilation Details

```
Profile:     bench (optimized)
Rust:        2024 Edition (Nightly)
Target:      Apple Silicon (aarch64)
```

## 📊 Summary Table (Serialization)

| Benchmark | serde_json | sonic_rs | kowito_json_jit | Winner | Gain |
|-----------|-----------|----------|-----------------|--------|------|
| tiny_3 (ns) | 34.2 | 21.6 | **13.0** | Kowito | 2.6x |
| medium_7 (ns) | 84.4 | 62.9 | **37.8** | Kowito | 2.2x |
| numeric_8 (ns) | 108.5 | 103.9 | **77.7** | Kowito | 1.4x |
| hot-loop (µs) | 94.5 | 70.8 | **47.9** | Kowito | 2.0x |
| large-string (ns) | 2731 | **284.6** | 401.9 | sonic_rs | - |
