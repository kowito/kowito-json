# Kowito-JSON Benchmark Report

**Date:** March 2, 2026  
**Version:** 0.2.10  
**Compiler:** Rust (2026-01-18)

---

## 📊 Executive Summary

Kowito-JSON demonstrates **superior performance** across serialization and parsing benchmarks. The NEON scanner reaches **6.8 GiB/s** on a 12 MB real-world JSON corpus — **5.3× faster** than sonic_rs and **29×** faster than serde_json at full parse throughput.

| Category | Winner | Speedup vs serde_json | Speedup vs sonic_rs |
|----------|--------|----------------------|---------------------|
| Tiny Payloads (3 fields) | **Kowito** | **3.1x** | **1.9x** |
| Medium Payloads (7 fields) | **Kowito** | **2.1x** | **1.7x** |
| Numeric Payloads (8 fields) | **Kowito** | **1.4x** | **1.2x** |
| Hot Loop (1000 items) | **Kowito** | **2.1x** | **1.6x** |
| Massive Parser – scanner only | **Kowito** | **29x** | **5.3x** |
| Massive Parser – schema JIT | **Kowito** | **29x** | **5.4x** |

---

## 1️⃣ Micro-Payload Serialization: tiny_3fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.48 GiB/s
sonic_rs        ███████ 2.34 GiB/s
kowito_json_jit ██████████████ 4.54 GiB/s
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      ████████████████ 34.3 ns
sonic_rs        ██████████ 21.7 ns
kowito_json_jit █████ 11.2 ns ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **3.1x faster** than serde_json, **1.9x faster** than sonic_rs

---

## 2️⃣ Micro-Payload Serialization: medium_7fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.36 GiB/s
sonic_rs        ███████ 1.76 GiB/s
kowito_json_jit ████████████ 3.07 GiB/s
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      █████████████████████ 81.1 ns
sonic_rs        █████████████████ 66.1 ns
kowito_json_jit ░░░░░░░░ 37.9 ns ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **2.1x faster** than serde_json, **1.7x faster** than sonic_rs

---

## 3️⃣ Micro-Payload Serialization: numeric_8fields

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 1.22 GiB/s
sonic_rs        █████ 1.45 GiB/s
kowito_json_jit ███████████ 1.76 GiB/s
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      ██████████████████ 118.9 ns
sonic_rs        ████████████████ 100.0 ns
kowito_json_jit ████████████ 82.4 ns ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **1.4x faster** than serde_json, **1.2x faster** than sonic_rs

---

## 4️⃣ Hot-Loop Serialization (1,000 items)

**Throughput Comparison (Higher is Better)**

```
serde_json      ████████████████ 1.19 GiB/s
sonic_rs        ████████████████████ 1.51 GiB/s
kowito_json_jit ████████████████████████████████ 2.46 GiB/s
```

**Latency (Microseconds - Lower is Better)**

```
serde_json      ████████████████████ 91.3 µs
sonic_rs        ████████████████ 72.3 µs
kowito_json_jit ████████████ 44.4 µs ⭐ FASTEST
```

**Performance Gain:** ✅ Kowito is **2.1x faster** than serde_json, **1.6x faster** than sonic_rs

---

## 5️⃣ Large-String Serialization (SIMD) - 10KB Strings

**Throughput Comparison (Higher is Better)**

```
serde_json      ████ 3.52 GiB/s
sonic_rs        ████████████████████████████████ 32.3 GiB/s ⭐ FASTEST
kowito_json_jit ███████████████████████ 24.3 GiB/s
```

**Latency (Nanoseconds - Lower is Better)**

```
serde_json      █████████████████████ 2649 ns
sonic_rs        ▌ 288.8 ns ⭐ FASTEST
kowito_json_jit  ░░░░░░░░░░░░░░░░░░░░░░░░ 383.6 ns
```

**Analysis:** For large strings, sonic_rs' specialized SIMD escape handling is optimal. Kowito remains competitive at **24.3 GiB/s**.

---

## 6️⃣ Massive JSON Parser – 12 MB Real-World Corpus (NEON baseline, aarch64)

> **Input:** 100,000 user-object JSON array, ~12.3 MiB  
> **Path:** NEON (ARM) — AVX2+PCLMULQDQ path requires x86_64 hardware  
> **Versions measured against:** serde_json 1.0, simd-json 0.17, sonic-rs 0.5

**Scanner Throughput (Higher is Better)**

```
serde_json              ▌ 234 MiB/s
simd_json               ▌ 265 MiB/s
sonic_rs                ████████ 1.29 GiB/s
kowito_scanner_only     ██████████████████████████████████████████ 6.84 GiB/s ⭐
kowito_schema_jit       ██████████████████████████████████████████ 6.93 GiB/s ⭐
```

**Latency (per 12 MB parse, Lower is Better)**

```
serde_json              ████████████████████████████████ 56.4 ms
simd_json               ████████████████████████████ 49.9 ms
sonic_rs                ██████ 10.1 ms
kowito_scanner_only     ▌ 1.89 ms ⭐ FASTEST
kowito_schema_jit       ▌ 1.87 ms ⭐ FASTEST
```

| Library | Time (ms) | Throughput | vs serde_json | vs sonic_rs |
|---|---|---|---|---|
| serde_json | 56.4 | 234 MiB/s | — | — |
| simd_json | 49.9 | 265 MiB/s | 1.1× | — |
| sonic_rs | 10.1 | 1.29 GiB/s | 5.6× | — |
| **kowito_scanner_only** | **1.89** | **6.84 GiB/s** | **29×** | **5.3×** |
| **kowito_schema_jit** | **1.87** | **6.93 GiB/s** | **29×** | **5.4×** |

**Performance Gain:** ✅ Kowito scanner is **29× faster** than serde_json,  
**5.3× faster** than sonic_rs (which uses its own SIMD parse path).

> **Note (x86_64):** The `scan_avx2` path (AVX2 + PCLMULQDQ, 64-byte blocks, implemented in v0.2.10) is not measured here. Expected throughput on x86_64 is **1.5–2.5× higher** than the generic scalar fallback that was used previously on that platform. A separate x86_64 run should be added once CI hardware is available.

---

## 📈 Performance Categories

### 🚀 Kowito Dominates
- **Tiny payloads** (< 50 bytes): 3.1x speedup
- **Medium payloads** (50-200 bytes): 2.1x speedup  
- **Repeated serialization** (1000+ items): 2.1x speedup

### ⚡ Highly Competitive
- **Numeric-heavy data**: 1.4x speedup
- **Large SIMD strings**: 24.3 GiB/s (competitive with sonic_rs)

---

## 🎯 Key Insights

### Why Kowito Wins on Micro/Hot-Loop Payloads

1. **Schema-JIT Compilation** - Bakes field layout at compile time
2. **Zero-Copy Template** - Static key prefixes avoid repeated formatting
3. **Branchless Path Selection** - No type introspection at runtime
4. **Implicit Capacity Reserve** - Single pre-allocation, no fragmentation

### Why sonic_rs Excels on Large Strings

- Uses NEON/AVX2 SIMD escape detection (lower constant overhead)
- Optimized for 100% escape scanning pass
- Kowito's hybrid approach trades SIMD specialization for generality

---

## 📋 Benchmark Methodology

- **Samples:** 100 measurements per benchmark
- **Confidence:** 95% (statistical significance: p < 0.05)
- **Profile:** `bench` (release optimizations enabled)
- **Outliers:** Detected and reported using criterion.rs

---

## 🔧 Compilation Details

```
Profile:     bench (optimized)
Rust:        2024 Edition + SIMD feature
Target:      Apple Silicon (aarch64)
SIMD:        NEON (always available on ARM64)
             AVX2+PCLMULQDQ implemented (v0.2.10); measured on x86_64 only
```

---

## 📊 Summary Table

### Serialization

| Benchmark | serde_json | sonic_rs | kowito_json_jit | Winner | Gain |
|-----------|-----------|----------|-----------------|--------|------|
| tiny_3 (ns) | 34.3 | 21.7 | **11.2** | Kowito | 3.1x |
| medium_7 (ns) | 81.1 | 66.1 | **37.9** | Kowito | 2.1x |
| numeric_8 (ns) | 118.9 | 100.0 | **82.4** | Kowito | 1.4x |
| hot-loop (µs) | 91.3 | 72.3 | **44.4** | Kowito | 2.1x |
| large-string (ns) | 2649 | **288.8** | 383.6 | sonic_rs | - |

### Parsing (aarch64 NEON, 12 MB corpus)

| Benchmark | serde_json | simd_json | sonic_rs | kowito | Winner | Gain vs sonic_rs |
|-----------|-----------|-----------|----------|--------|--------|------------------|
| scanner_only (ms) | 56.4 | 49.9 | 10.1 | **1.89** | Kowito | **5.3×** |
| schema_jit (ms) | 56.4 | 49.9 | 10.1 | **1.87** | Kowito | **5.4×** |
| scanner_only (GiB/s) | 0.23 | 0.26 | 1.29 | **6.84** | Kowito | **5.3×** |

---

## 🎓 Conclusion

**Kowito-JSON** is the optimal choice for:
- ✅ Microservices with small, repeated payloads
- ✅ Hot-path JSON serialization
- ✅ Memory-constrained environments
- ✅ Real-time systems requiring predictable latency

**sonic_rs** remains superior for:
- ✅ Large string-heavy workloads (> 10KB)
- ✅ Pure throughput maximization on SIMD platforms

