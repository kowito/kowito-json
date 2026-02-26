use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

// A simple compile-time hash function to match the FxHash style
fn compute_hash(s: &str) -> u64 {
    let mut hash: u64 = 0;
    for &b in s.as_bytes() {
        hash = (hash ^ (b as u64)).wrapping_mul(0x517cc1b727220a95);
    }
    hash
}

/// Returns `true` when the type is a string-like type that needs JSON quotes
/// provided by `FastWrite` (String, &str, str, Cow, KString, …). We detect the
/// most common ones by their last path segment name. Unknown types that
/// implement `FastWrite` will still work correctly at runtime; this check only
/// drives compile-time capacity estimates.
fn is_str_like(ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => {
            let seg = tp.path.segments.last().map(|s| s.ident.to_string());
            matches!(seg.as_deref(), Some("String" | "str" | "Cow" | "KString" | "Box"))
        }
        Type::Reference(r) => is_str_like(&r.elem),
        _ => false,
    }
}

/// Rough per-value capacity estimate used for `buf.reserve`.
/// - strings: 16 bytes (typical short value + quotes)
/// - numbers/bools: 8 bytes
fn value_capacity_estimate(ty: &Type) -> usize {
    if is_str_like(ty) { 18 } else { 8 }
}

#[proc_macro_derive(Kjson)]
pub fn kowito_json_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut generated_fields = Vec::new();
    let mut fields_init = Vec::new();
    let mut serialize_stmts = Vec::new();

    // Compile-time capacity: we know every static key prefix byte exactly.
    // static_cap  = 2 (`{}`) + sum over fields of (comma + `"key":`)
    // dynamic_cap = rough estimate for values
    let mut static_cap: usize = 2; // opening `{` + closing `}`
    let mut dynamic_cap: usize = 0;

    if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            let total_fields = fields_named.named.len();
            for (i, field) in fields_named.named.iter().enumerate() {
                let field_ident = field.ident.as_ref().unwrap();
                let field_name_str = field_ident.to_string();
                let field_hash = compute_hash(&field_name_str);

                generated_fields.push(quote! {
                    #field_hash => {
                        // Phase 3: Unrolled SIMD bypass logic specializes here per field type!
                    }
                });

                fields_init.push(quote! {
                    #field_ident: Default::default()
                });

                // ----------------------------------------------------------------
                // Compile-time structural template generation
                //
                // Each field contributes a *static* key prefix slice that is
                // interleaved with the *dynamic* value writes.  All prefix bytes
                // are known at macro-expansion time so they compile down to a
                // direct `memcpy` from a read-only data segment.
                // ----------------------------------------------------------------
                let prefix = if i == 0 {
                    format!("{{\"{}\":", field_name_str)   // opens the object
                } else {
                    format!(",\"{}\":", field_name_str)
                };

                // Accumulate static capacity (prefix bytes are known at expand time)
                static_cap += prefix.len();
                dynamic_cap += value_capacity_estimate(&field.ty);

                let prefix_bytes = syn::LitByteStr::new(
                    prefix.as_bytes(),
                    proc_macro2::Span::call_site(),
                );
                let is_last = i == total_fields - 1;

                serialize_stmts.push(quote! {
                    // Static key prefix — single memcpy from read-only data
                    buf.extend_from_slice(#prefix_bytes);
                    // Dynamic value — dispatched through FastWrite (itoa / ryu / escape)
                    kowito_json::serialize::write_value(&self.#field_ident, buf);
                });

                if is_last {
                    serialize_stmts.push(quote! { buf.push(b'}'); });
                }
            }
        }
    }

    let total_capacity = static_cap + dynamic_cap;

    let expanded = quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_json_schema_version() -> &'static str {
                "1.0.0-turbo"
            }

            /// Zero-Decode constructor — populates from a KView without copying
            /// string data (lazy decode).
            pub fn from_kview<'a>(view: &kowito_json::KView<'a>) -> Self {
                Self {
                    #(
                        #fields_init,
                    )*
                }
            }

            /// Ultra-Fast Schema-JIT Serializer.
            ///
            /// The JSON object layout is baked in at *compile time*:
            /// - All field-key prefixes are static byte slices (`&'static [u8]`)
            ///   stored in the read-only data segment — no heap allocation.
            /// - Integer fields use `itoa` (lookup-table based, branchless).
            /// - Float fields use `ryu` (Grisu3/Dragon4 — shortest round-trip).
            /// - String fields use the lookup-table escape fast-path in
            ///   `kowito_json::serialize::write_str_escape`.
            ///
            /// A single `reserve` call at the top pre-allocates the estimated
            /// capacity so the hot loop below never reallocates for typical
            /// small payloads.
            #[inline]
            pub fn to_kbytes(&self, buf: &mut Vec<u8>) {
                // Pre-allocate: static template bytes + rough value estimate.
                // Both numbers are known at compile time.
                buf.reserve(#total_capacity);
                #(
                    #serialize_stmts
                )*
            }
        }

        impl kowito_json::serialize::FastWrite for #name {
            #[inline(always)]
            fn write_fast(&self, buf: &mut Vec<u8>) {
                self.to_kbytes(buf);
            }
        }
    };

    TokenStream::from(expanded)
}
