use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Fields, Lit, Meta, Token, Type,
    parse_macro_input,
    punctuated::Punctuated,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn field_name_hash(s: &str) -> u64 {
    let mut hash: u64 = 0;
    for &b in s.as_bytes() {
        hash = (hash ^ (b as u64)).wrapping_mul(0x517cc1b727220a95);
    }
    hash
}

fn is_string_type(ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => {
            let seg = tp.path.segments.last().map(|s| s.ident.to_string());
            matches!(
                seg.as_deref(),
                Some("String" | "str" | "Cow" | "KString" | "Box")
            )
        }
        Type::Reference(r) => is_string_type(&r.elem),
        _ => false,
    }
}

fn value_capacity_estimate(ty: &Type) -> usize {
    if is_string_type(ty) { 18 } else { 8 }
}

// ---------------------------------------------------------------------------
// #[kjson(...)] attribute parsing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct KJsonAttrs {
    rename: Option<String>,
    skip: bool,
    skip_serializing_if: Option<syn::Path>,
}

fn parse_kjson_attrs(attrs: &[syn::Attribute]) -> KJsonAttrs {
    let mut out = KJsonAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("kjson") {
            continue;
        }
        // Parse #[kjson(key = "value", skip, ...)]
        let nested = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .unwrap_or_default();
        for meta in nested {
            match &meta {
                Meta::NameValue(nv) if nv.path.is_ident("rename") => {
                    if let syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value {
                        out.rename = Some(s.value());
                    }
                }
                Meta::Path(p) if p.is_ident("skip") => {
                    out.skip = true;
                }
                Meta::NameValue(nv) if nv.path.is_ident("skip_serializing_if") => {
                    if let syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value {
                        if let Ok(path) = s.parse::<syn::Path>() {
                            out.skip_serializing_if = Some(path);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Named struct serialization (existing schema-JIT fast path, extended)
// ---------------------------------------------------------------------------

fn gen_named_struct(
    name: &syn::Ident,
    fields_named: &syn::FieldsNamed,
) -> proc_macro2::TokenStream {
    let mut fields_init = Vec::new();
    let mut serialize_stmts = Vec::new();
    let mut dynamic_cap_stmts = Vec::new();
    let mut serde_field_stmts = Vec::new();
    let mut static_cap: usize = 2; // `{` + `}`

    // Count non-skipped fields for index tracking
    let visible_fields: Vec<_> = fields_named
        .named
        .iter()
        .filter(|f| !parse_kjson_attrs(&f.attrs).skip)
        .collect();
    let total_visible = visible_fields.len();

    let mut visible_idx = 0usize;
    for field in &fields_named.named {
        let field_ident = field.ident.as_ref().unwrap();
        let attrs = parse_kjson_attrs(&field.attrs);

        fields_init.push(quote! { #field_ident: Default::default() });

        if attrs.skip {
            continue;
        }

        let key_owned;
        let key = if let Some(r) = &attrs.rename {
            r.as_str()
        } else {
            key_owned = field_ident.to_string();
            key_owned.as_str()
        };
        let prefix = if visible_idx == 0 {
            format!("{{\"{}\":", key)
        } else {
            format!(",\"{}\":", key)
        };
        static_cap += prefix.len();
        visible_idx += 1;

        if is_string_type(&field.ty) {
            dynamic_cap_stmts.push(quote! { self.#field_ident.len() * 6 + 2 });
        } else {
            let cap = value_capacity_estimate(&field.ty);
            dynamic_cap_stmts.push(quote! { #cap });
        }

        let prefix_bytes =
            syn::LitByteStr::new(prefix.as_bytes(), proc_macro2::Span::call_site());
        let is_last = visible_idx == total_visible;

        let inner = if let Some(ref cond_path) = attrs.skip_serializing_if {
            quote! {
                if !#cond_path(&self.#field_ident) {
                    std::ptr::copy_nonoverlapping(#prefix_bytes.as_ptr(), curr, #prefix_bytes.len());
                    curr = curr.add(#prefix_bytes.len());
                    curr = kowito_json::serialize::write_value_raw(&self.#field_ident, curr);
                }
            }
        } else {
            quote! {
                std::ptr::copy_nonoverlapping(#prefix_bytes.as_ptr(), curr, #prefix_bytes.len());
                curr = curr.add(#prefix_bytes.len());
                curr = kowito_json::serialize::write_value_raw(&self.#field_ident, curr);
            }
        };

        serialize_stmts.push(inner);

        if is_last {
            serialize_stmts.push(quote! {
                *curr = b'}';
                curr = curr.add(1);
            });
        }

        // serde field serialization
        let key_str = key;
        if let Some(ref cond_path) = attrs.skip_serializing_if {
            serde_field_stmts.push(quote! {
                if #cond_path(&self.#field_ident) {
                    serde::ser::SerializeStruct::skip_field(&mut _serde_state, #key_str)?;
                } else {
                    serde::ser::SerializeStruct::serialize_field(&mut _serde_state, #key_str, &self.#field_ident)?;
                }
            });
        } else {
            serde_field_stmts.push(quote! {
                serde::ser::SerializeStruct::serialize_field(&mut _serde_state, #key_str, &self.#field_ident)?;
            });
        }
    }

    // Handle all-skipped edge-case: emit empty object
    if total_visible == 0 {
        serialize_stmts.push(quote! {
            std::ptr::copy_nonoverlapping(b"{}".as_ptr(), curr, 2);
            curr = curr.add(2);
        });
    }

    let _field_hash_arms: Vec<_> = fields_named.named.iter().map(|f| {
        let fi = f.ident.as_ref().unwrap();
        let hash = field_name_hash(&fi.to_string());
        quote! { #hash => {} }
    }).collect();

    quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_json_schema_version() -> &'static str {
                "1.0.0-turbo"
            }

            pub fn from_kview<'a>(view: &kowito_json::KView<'a>) -> Self {
                let _ = view;
                Self { #( #fields_init, )* }
            }

            #[inline]
            pub fn to_json_bytes(&self, buf: &mut Vec<u8>) {
                let old_len = buf.len();
                let mut dynamic_cap = 0usize;
                #( dynamic_cap += #dynamic_cap_stmts; )*
                buf.reserve(#static_cap + dynamic_cap);
                unsafe {
                    let mut curr = buf.as_mut_ptr().add(old_len);
                    #( #serialize_stmts )*
                    buf.set_len(curr.offset_from(buf.as_ptr()) as usize);
                }
            }
        }

        impl kowito_json::serialize::Serialize for #name {
            #[inline(always)]
            fn serialize(&self, buf: &mut Vec<u8>) {
                self.to_json_bytes(buf);
            }
        }

        impl kowito_json::serialize::SerializeRaw for #name {
            #[inline(always)]
            unsafe fn serialize_raw(&self, mut curr: *mut u8) -> *mut u8 {
                #( #serialize_stmts )*
                curr
            }
        }

        impl serde::Serialize for #name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                let mut _serde_state = serde::Serializer::serialize_struct(
                    serializer, stringify!(#name), #total_visible)?;
                #( #serde_field_stmts )*
                serde::ser::SerializeStruct::end(_serde_state)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Newtype struct  Foo(T)
// ---------------------------------------------------------------------------

fn gen_newtype_struct(name: &syn::Ident, field: &syn::Field) -> proc_macro2::TokenStream {
    let ty = &field.ty;
    let cap = value_capacity_estimate(ty);
    quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_json_schema_version() -> &'static str {
                "1.0.0-turbo"
            }

            #[inline]
            pub fn to_json_bytes(&self, buf: &mut Vec<u8>) {
                buf.reserve(#cap);
                kowito_json::serialize::write_value(&self.0, buf);
            }
        }

        impl kowito_json::serialize::Serialize for #name {
            #[inline(always)]
            fn serialize(&self, buf: &mut Vec<u8>) {
                self.to_json_bytes(buf);
            }
        }

        impl kowito_json::serialize::SerializeRaw for #name {
            #[inline(always)]
            unsafe fn serialize_raw(&self, curr: *mut u8) -> *mut u8 {
                unsafe { kowito_json::serialize::write_value_raw(&self.0, curr) }
            }
        }

        impl serde::Serialize for #name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tuple struct  Foo(A, B, C)
// ---------------------------------------------------------------------------

fn gen_tuple_struct(name: &syn::Ident, fields: &syn::FieldsUnnamed) -> proc_macro2::TokenStream {
    let indices: Vec<syn::Index> = (0..fields.unnamed.len())
        .map(syn::Index::from)
        .collect();
    let types: Vec<_> = fields.unnamed.iter().map(|f| &f.ty).collect();
    let caps: Vec<usize> = types.iter().map(|t| value_capacity_estimate(t)).collect();
    let total_cap: usize = caps.iter().sum::<usize>() + fields.unnamed.len() + 2;
    let len = fields.unnamed.len();

    quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_json_schema_version() -> &'static str {
                "1.0.0-turbo"
            }

            #[inline]
            pub fn to_json_bytes(&self, buf: &mut Vec<u8>) {
                buf.reserve(#total_cap);
                buf.push(b'[');
                #(
                    if #indices > 0 { buf.push(b','); }
                    kowito_json::serialize::write_value(&self.#indices, buf);
                )*
                buf.push(b']');
            }
        }

        impl kowito_json::serialize::Serialize for #name {
            #[inline(always)]
            fn serialize(&self, buf: &mut Vec<u8>) {
                self.to_json_bytes(buf);
            }
        }

        impl serde::Serialize for #name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                use serde::ser::SerializeTupleStruct;
                let mut state = serializer.serialize_tuple_struct(stringify!(#name), #len)?;
                #( state.serialize_field(&self.#indices)?; )*
                state.end()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit struct  Foo
// ---------------------------------------------------------------------------

fn gen_unit_struct(name: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_json_schema_version() -> &'static str {
                "1.0.0-turbo"
            }

            #[inline]
            pub fn to_json_bytes(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(b"null");
            }
        }

        impl kowito_json::serialize::Serialize for #name {
            #[inline(always)]
            fn serialize(&self, buf: &mut Vec<u8>) {
                self.to_json_bytes(buf);
            }
        }

        impl serde::Serialize for #name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                serializer.serialize_unit_struct(stringify!(#name))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Enum  (externally tagged — matches serde_json default)
// ---------------------------------------------------------------------------

fn gen_enum(name: &syn::Ident, data: &DataEnum) -> proc_macro2::TokenStream {
    let mut to_json_arms = Vec::new();
    let mut serde_arms = Vec::new();

    for (vi, variant) in data.variants.iter().enumerate() {
        let vident = &variant.ident;
        let variant_name = vident.to_string();
        let vi_u32 = vi as u32;

        match &variant.fields {
            // Unit variant → "VariantName"
            Fields::Unit => {
                let quoted = format!("\"{}\"", variant_name);
                let quoted_bytes =
                    syn::LitByteStr::new(quoted.as_bytes(), proc_macro2::Span::call_site());
                to_json_arms.push(quote! {
                    #name::#vident => {
                        buf.extend_from_slice(#quoted_bytes);
                    }
                });
                serde_arms.push(quote! {
                    #name::#vident => {
                        serializer.serialize_unit_variant(stringify!(#name), #vi_u32, #variant_name)
                    }
                });
            }

            // Newtype variant → {"VariantName":value}
            Fields::Unnamed(fu) if fu.unnamed.len() == 1 => {
                let prefix = format!("{{\"{}\":", variant_name);
                let prefix_bytes =
                    syn::LitByteStr::new(prefix.as_bytes(), proc_macro2::Span::call_site());
                to_json_arms.push(quote! {
                    #name::#vident(inner) => {
                        buf.extend_from_slice(#prefix_bytes);
                        kowito_json::serialize::write_value(inner, buf);
                        buf.push(b'}');
                    }
                });
                serde_arms.push(quote! {
                    #name::#vident(inner) => {
                        serializer.serialize_newtype_variant(stringify!(#name), #vi_u32, #variant_name, inner)
                    }
                });
            }

            // Tuple variant → {"VariantName":[a,b,...]}
            Fields::Unnamed(fu) => {
                let indices: Vec<syn::Ident> = (0..fu.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("_f{}", i), proc_macro2::Span::call_site()))
                    .collect();
                let prefix = format!("{{\"{}\":[", variant_name);
                let prefix_bytes =
                    syn::LitByteStr::new(prefix.as_bytes(), proc_macro2::Span::call_site());
                let len = fu.unnamed.len();
                to_json_arms.push(quote! {
                    #name::#vident(#( #indices ),*) => {
                        buf.extend_from_slice(#prefix_bytes);
                        let mut _first = true;
                        #(
                            if !_first { buf.push(b','); }
                            _first = false;
                            kowito_json::serialize::write_value(#indices, buf);
                        )*
                        buf.extend_from_slice(b"]}");
                    }
                });
                serde_arms.push(quote! {
                    #name::#vident(#( #indices ),*) => {
                        use serde::ser::SerializeTupleVariant;
                        let mut state = serializer.serialize_tuple_variant(
                            stringify!(#name), #vi_u32, #variant_name, #len)?;
                        #( state.serialize_field(#indices)?; )*
                        state.end()
                    }
                });
            }

            // Struct variant → {"VariantName":{"field":value,...}}
            Fields::Named(fn_) => {
                let fidents: Vec<_> =
                    fn_.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                let fnames: Vec<String> = fidents.iter().map(|i| i.to_string()).collect();
                let prefix = format!("{{\"{}\":{{", variant_name);
                let prefix_bytes =
                    syn::LitByteStr::new(prefix.as_bytes(), proc_macro2::Span::call_site());
                let len = fidents.len();

                let field_stmts: Vec<_> = fidents.iter().zip(fnames.iter()).enumerate().map(|(i, (fi, fn_))| {
                    let sep = if i == 0 {
                        format!("\"{}\":", fn_)
                    } else {
                        format!(",\"{}\":", fn_)
                    };
                    let sep_bytes =
                        syn::LitByteStr::new(sep.as_bytes(), proc_macro2::Span::call_site());
                    quote! {
                        buf.extend_from_slice(#sep_bytes);
                        kowito_json::serialize::write_value(#fi, buf);
                    }
                }).collect();

                to_json_arms.push(quote! {
                    #name::#vident { #( #fidents ),* } => {
                        buf.extend_from_slice(#prefix_bytes);
                        #( #field_stmts )*
                        buf.extend_from_slice(b"}}");
                    }
                });

                serde_arms.push(quote! {
                    #name::#vident { #( #fidents ),* } => {
                        use serde::ser::SerializeStructVariant;
                        let mut state = serializer.serialize_struct_variant(
                            stringify!(#name), #vi_u32, #variant_name, #len)?;
                        #( state.serialize_field(#fnames, #fidents)?; )*
                        state.end()
                    }
                });
            }
        }
    }

    quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_json_schema_version() -> &'static str {
                "1.0.0-turbo"
            }

            #[inline]
            pub fn to_json_bytes(&self, buf: &mut Vec<u8>) {
                match self {
                    #( #to_json_arms )*
                }
            }
        }

        impl kowito_json::serialize::Serialize for #name {
            #[inline(always)]
            fn serialize(&self, buf: &mut Vec<u8>) {
                self.to_json_bytes(buf);
            }
        }

        impl serde::Serialize for #name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
                match self {
                    #( #serde_arms )*
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Deserialize impl generation
// ---------------------------------------------------------------------------

fn gen_named_struct_deser(
    name: &syn::Ident,
    fields_named: &syn::FieldsNamed,
) -> proc_macro2::TokenStream {
    let visible_fields: Vec<_> = fields_named
        .named
        .iter()
        .filter(|f| !parse_kjson_attrs(&f.attrs).skip)
        .collect();

    // For each field, generate:
    //   let mut _field_x: Option<TypeX> = None;
    let mut option_decls = Vec::new();
    // Match arms: "field_name" => { _field_x = Some(...parse...); }
    let mut match_arms = Vec::new();
    // Final struct construction: field_x: _field_x.unwrap_or_default()
    let mut field_inits = Vec::new();

    for field in &visible_fields {
        let fi = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let attrs = parse_kjson_attrs(&field.attrs);
        let key = attrs.rename.unwrap_or_else(|| fi.to_string());
        let opt_ident = syn::Ident::new(&format!("_field_{}", fi), proc_macro2::Span::call_site());

        option_decls.push(quote! {
            let mut #opt_ident: Option<#ty> = None;
        });

        match_arms.push(quote! {
            #key => {
                _parser.expect_colon()?;
                #opt_ident = Some(<#ty as kowito_json::Deserialize>::deserialize(_parser)?);
            }
        });

        field_inits.push(quote! {
            #fi: #opt_ident.unwrap_or_default(),
        });
    }

    // Also handle skipped fields (default)
    let skipped_fields: Vec<_> = fields_named
        .named
        .iter()
        .filter(|f| parse_kjson_attrs(&f.attrs).skip)
        .collect();
    for field in &skipped_fields {
        let fi = field.ident.as_ref().unwrap();
        field_inits.push(quote! { #fi: Default::default(), });
    }

    quote! {
        impl kowito_json::Deserialize for #name {
            fn deserialize(_parser: &mut kowito_json::Parser<'_>) -> kowito_json::Result<Self> {
                _parser.begin_object()?;

                #( #option_decls )*

                // Check for empty object
                if let Some(tok) = _parser.tape.get(_parser.pos).map(|t| t & 0xF000_0000) {
                    if tok == (3 << 28) {
                        _parser.pos += 1;
                        return Ok(Self { #( #field_inits )* });
                    }
                }

                loop {
                    let key = _parser.parse_string_owned()?;
                    match key.as_str() {
                        #( #match_arms )*
                        _ => {
                            _parser.expect_colon()?;
                            _parser.skip_value()?;
                        }
                    }
                    if !_parser.object_next()? {
                        break;
                    }
                }

                Ok(Self { #( #field_inits )* })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Enum deserialization
// ---------------------------------------------------------------------------
fn gen_enum_deser(name: &syn::Ident, data: &DataEnum) -> proc_macro2::TokenStream {
    let mut match_arms = Vec::new();
    for variant in data.variants.iter() {
        let vident = &variant.ident;
        let variant_name = vident.to_string();
        match &variant.fields {
            Fields::Unit => {
                match_arms.push(quote! {
                    #variant_name => Ok(#name::#vident),
                });
            }
            Fields::Unnamed(fu) if fu.unnamed.len() == 1 => {
                match_arms.push(quote! {
                    #variant_name => {
                        _parser.expect_colon()?;
                        let inner = <_ as kowito_json::Deserialize>::deserialize(_parser)?;
                        Ok(#name::#vident(inner))
                    },
                });
            }
            Fields::Unnamed(fu) => {
                let len = fu.unnamed.len();
                let indices: Vec<_> = (0..len).map(|i| syn::Ident::new(&format!("_f{i}"), proc_macro2::Span::call_site())).collect();
                let field_tys: Vec<_> = fu.unnamed.iter().map(|f| &f.ty).collect();
                // Each element must call array_next() after deserializing to consume ',' or ']'
                let deser_pairs: Vec<_> = indices.iter().zip(field_tys.iter()).map(|(id, ty)| {
                    quote! {
                        let #id = <#ty as kowito_json::Deserialize>::deserialize(_parser)?;
                        _parser.array_next()?;
                    }
                }).collect();
                match_arms.push(quote! {
                    #variant_name => {
                        _parser.expect_colon()?;
                        _parser.begin_array()?;
                        #( #deser_pairs )*
                        Ok(#name::#vident(#( #indices ),*))
                    },
                });
            }
            Fields::Named(fn_) => {
                // For struct variants, manually deserialize each field and construct the variant
                let field_idents: Vec<_> = fn_.named.iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let field_tys: Vec<_> = fn_.named.iter()
                    .map(|f| &f.ty)
                    .collect();
                let field_strs: Vec<String> = field_idents.iter()
                    .map(|id| id.to_string())
                    .collect();
                let opt_idents: Vec<_> = field_idents.iter()
                    .map(|id| syn::Ident::new(&format!("_vf_{id}"), proc_macro2::Span::call_site()))
                    .collect();
                match_arms.push(quote! {
                    #variant_name => {
                        _parser.expect_colon()?;
                        _parser.begin_object()?;
                        #( let mut #opt_idents: Option<#field_tys> = None; )*
                        // Check for empty object
                        if _parser.tape.get(_parser.pos).map(|t| t & 0xF000_0000) != Some(3 << 28) {
                            loop {
                                let _key = _parser.parse_string_owned()?;
                                match _key.as_str() {
                                    #(
                                        #field_strs => {
                                            _parser.expect_colon()?;
                                            #opt_idents = Some(<#field_tys as kowito_json::Deserialize>::deserialize(_parser)?);
                                        }
                                    )*
                                    _ => {
                                        _parser.expect_colon()?;
                                        _parser.skip_value()?;
                                    }
                                }
                                if !_parser.object_next()? { break; }
                            }
                        } else {
                            _parser.pos += 1;
                        }
                        Ok(#name::#vident { #( #field_idents: #opt_idents.unwrap_or_default() ),* })
                    },
                });
            }
        }
    }
    // Collect unit variant names for the plain-string form (unit variants serialize as "Name")
    let unit_match_arms: Vec<_> = data.variants.iter().filter_map(|v| {
        if matches!(v.fields, Fields::Unit) {
            let vident = &v.ident;
            let vname = vident.to_string();
            Some(quote! { #vname => Ok(#name::#vident), })
        } else {
            None
        }
    }).collect();

    quote! {
        impl kowito_json::Deserialize for #name {
            fn deserialize(_parser: &mut kowito_json::Parser<'_>) -> kowito_json::Result<Self> {
                // Unit variants are serialized as plain strings; others as {"Variant": ...}
                if _parser.peek_is_string() {
                    let key = _parser.parse_string_owned()?;
                    match key.as_str() {
                        #( #unit_match_arms )*
                        _ => Err(kowito_json::Error::custom("unknown unit enum variant")),
                    }
                } else {
                    _parser.begin_object()?;
                    let key = _parser.parse_string_owned()?;
                    match key.as_str() {
                        #( #match_arms )*
                        _ => Err(kowito_json::Error::custom("unknown enum variant")),
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[proc_macro_derive(KJson, attributes(kjson))]
pub fn kowito_json_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = match &input.data {
        Data::Struct(DataStruct { fields: Fields::Named(fn_), .. }) => {
            let ser = gen_named_struct(name, fn_);
            let deser = gen_named_struct_deser(name, fn_);
            quote! { #ser #deser }
        }
        Data::Struct(DataStruct { fields: Fields::Unnamed(fu), .. }) => {
            if fu.unnamed.len() == 1 {
                gen_newtype_struct(name, fu.unnamed.first().unwrap())
            } else {
                gen_tuple_struct(name, fu)
            }
        }
        Data::Struct(DataStruct { fields: Fields::Unit, .. }) => {
            gen_unit_struct(name)
        }
        Data::Enum(de) => {
            let ser = gen_enum(name, de);
            let deser = gen_enum_deser(name, de);
            quote! { #ser #deser }
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(name, "KJson does not support unions")
                .to_compile_error()
                .into();
        }
    };

    TokenStream::from(expanded)
}

