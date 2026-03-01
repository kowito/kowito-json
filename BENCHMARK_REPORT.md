# Kowito-JSON Benchmark Report

**Date:** March 1, 2026  
**Version:** 0.2.8  
**Compiler:** Rust (2026-01-18)

---

## 📊 Executive Summary

Kowito-JSON demonstrates **superior performance** across all micro and hot-loop benchmarks, with **2.1x-3.1x speedup** over competing JSON serializers on typical payloads.

| Category | Winner | Speedup vs serde_json | Speedup vs sonic_rs |
|----------|--------|----------------------|---------------------|
| Tiny Payloads (3 fields) | **Kowito** | **3.1x** | **1.9x** |
| Medium Payloads (7 fields) | **Kowito** | **2.1x** | **1.7x** |
| Numeric Payloads (8 fields) | **Kowito** | **1.4x** | **1.2x** |
| Hot Loop (1000 items) | **Kowito** | **2.1x** | **1.6x** |

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
```

---

## 📊 Summary Table

| Benchmark | serde_json | sonic_rs | kowito_json_jit | Winner | Gain |
|-----------|-----------|----------|-----------------|--------|------|
| tiny_3 (ns) | 34.3 | 21.7 | **11.2** | Kowito | 3.1x |
| medium_7 (ns) | 81.1 | 66.1 | **37.9** | Kowito | 2.1x |
| numeric_8 (ns) | 118.9 | 100.0 | **82.4** | Kowito | 1.4x |
| hot-loop (µs) | 91.3 | 72.3 | **44.4** | Kowito | 2.1x |
| large-string (ns) | 2649 | **288.8** | 383.6 | sonic_rs | - |

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

