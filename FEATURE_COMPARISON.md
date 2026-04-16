# Feature Comparison: kowito-json vs serde_json

Legend: ✅ Supported · ❌ Not supported · ⚠️ Partial · 🚧 Planned

---

## Serialization

| Feature | kowito-json | serde_json | Notes |
|:---|:---:|:---:|:---|
| Struct serialization | ✅ | ✅ | kowito via `#[derive(KJson)]`; serde via `Serialize` |
| Enum serialization | ✅ | ✅ | all 4 variant kinds: unit / newtype / tuple / struct |
| HashMap / BTreeMap | ✅ | ✅ | via `serde::Serialize` path |
| Tuple / tuple struct | ✅ | ✅ | |
| Unit struct / newtype wrapper | ✅ | ✅ | |
| `Vec<T>` / slices | ✅ | ✅ | |
| `Option<T>` | ✅ | ✅ | |
| `Box<T>` / `Cow<str>` | ✅ | ✅ | |
| Primitive integers (i8–i64, u8–u64) | ✅ | ✅ | |
| f32 / f64 | ✅ | ✅ | kowito uses `ryu`; serde uses `ryu` too |
| bool | ✅ | ✅ | |
| String with SIMD escape scanning | ✅ | ❌ | kowito: NEON / AVX2 |
| Pretty-print output | ✅ | ✅ | `to_string_pretty` / `to_writer_pretty` |
| Write to `io::Write` / streaming output | ✅ | ✅ | `to_writer` / `to_writer_pretty` |
| Compile-time key baking (Schema-JIT) | ✅ | ❌ | kowito bakes `"field":` as `&'static [u8]` |
| Custom serializer impl (manual) | ✅ | ✅ | kowito: `Serialize` + `SerializeRaw` traits |
| `serde::Serialize` ecosystem compatibility | ✅ | ✅ | `KowitoSerializer<W,F>` implements `serde::Serializer` |

---

## Parsing / Deserialization

| Feature | kowito-json | serde_json | Notes |
|:---|:---:|:---:|:---|
| Struct deserialization | ✅ | ✅ | `#[derive(KJson)]` now generates `Deserialize` impl; supports field rename/skip |
| Enum deserialization | ✅ | ✅ | all 4 variant kinds: unit / newtype / tuple / struct; unit variants handle plain-string form |
| HashMap / BTreeMap | ✅ | ✅ | |
| `serde::Deserialize` ecosystem compatibility | ❌ | ✅ | |
| Zero-decode / lazy field access | ✅ | ❌ | kowito scans to tape without decoding values |
| Full-document `Value` type | ✅ | ✅ | `kowito_json::Value` (Null/Bool/Number/Str/Array/Object) |
| Structural tape (u32 token stream) | ✅ | ❌ | kowito: flat `Vec<u32>` tape |
| SIMD structural scanning | ✅ | ❌ | kowito: PMULL / AVX2+PCLMULQDQ |
| Arena / zero-allocation parse | ✅ | ❌ | kowito: `Scratchpad` + `with_scratch_tape` |
| Random-access field lookup (KView) | ⚠️ | ❌ | `KView` API exists but is early-stage |
| Number lazily decoded from raw bytes | ✅ | ❌ | kowito: `KNode::Number(&[u8])` |
| String lazily decoded (`KString`) | ✅ | ❌ | kowito: zero-copy, decode only on access |
| UTF-8 validation | ✅ | ✅ | `from_slice` validates; `from_str` assumes valid |
| Detailed error messages (line / column) | ✅ | ✅ | `Error::Parse { msg, line, col }` |
| Streaming / incremental parse | ❌ | ✅ | |
| `from_str` / `from_slice` convenience API | ✅ | ✅ | `kowito_json::from_str` / `from_slice` now available |

---

## Runtime & Platform

| Feature | kowito-json | serde_json | Notes |
|:---|:---:|:---:|:---|
| Stable Rust | ❌ | ✅ | kowito requires nightly (`portable_simd`, intrinsics) |
| `no_std` + `alloc` | ❌ | ✅ | kowito uses `thread_local!`, `std` features |
| ARM64 NEON (Apple Silicon / Graviton) | ✅ | ❌ | |
| x86_64 AVX2 + PCLMULQDQ | ✅ | ❌ | |
| SVE2 (AArch64 v9) | 🚧 | ❌ | prototype in `scanner/sve2.rs` |
| AMX (Apple Matrix coprocessor) | 🚧 | ❌ | prototype in `scanner/amx.rs` |
| Generic portable SIMD fallback | ✅ | ❌ | |
| Universal architecture support | ❌ | ✅ | serde_json compiles anywhere |
| Thread-local scratchpad | ✅ | ❌ | `GLOBAL_SCRATCHPAD` (1M entries) |

---

## Developer Experience

| Feature | kowito-json | serde_json | Notes |
|:---|:---:|:---:|:---|
| Derive macro | ✅ | ✅ | kowito: `#[derive(KJson)]`; serde: `#[derive(Serialize, Deserialize)]` |
| Field rename (`#[serde(rename)]`) | ✅ | ✅ | `#[kjson(rename = "name")]` |
| Field skip (`#[serde(skip)]`) | ✅ | ✅ | `#[kjson(skip)]` / `#[kjson(skip_serializing_if = "fn")]` |
| Default values (`#[serde(default)]`) | ❌ | ✅ | |
| Flatten (`#[serde(flatten)]`) | ❌ | ✅ | |
| Custom deserializer hooks | ❌ | ✅ | |
| JSON path / pointer | ❌ | ✅ | `serde_json::Value::pointer()` |
| `json!` macro | ✅ | ✅ | `json!({ "key": value, "arr": [1, 2] })` |
| Merge / patch JSON | ❌ | ✅ | `json_patch` crate via serde |
| Crates.io maturity | Early | Stable | serde_json: 600M+ downloads |
| Documentation coverage | Partial | Extensive | |

---

## Performance Summary

| Metric | kowito-json | serde_json |
|:---|:---:|:---:|
| Parsing throughput (12 MB corpus) | **~7.98 GiB/s** | ~0.241 GiB/s |
| Micro serialization (3 fields) | **11.2 ns** | 34.3 ns |
| Hot-loop serialization (1000 items) | **44.4 µs** | 91.3 µs |
| Large-string serialization (10 KB) | 383.6 ns | 2649 ns |

---

## When to Choose Each

| Scenario | Recommendation |
|:---|:---|
| Maximum throughput, known schema, nightly Rust | **kowito-json** |
| Arbitrary / dynamic JSON documents | serde_json |
| Enum variants in JSON payload | **kowito-json** |
| Stable Rust / `no_std` targets | serde_json |
| Serde ecosystem interop (`#[serde(...)]` attrs) | serde_json |
| Microservice hot-path with fixed struct schema | **kowito-json** |
| Logging pipelines, real-time serialization | **kowito-json** |
| General-purpose production use today | serde_json |
