use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

// A simple compile-time hash function to match the FxHash style
fn compute_hash(s: &str) -> u64 {
    let mut hash: u64 = 0;
    for &b in s.as_bytes() {
        hash = (hash ^ (b as u64)).wrapping_mul(0x517cc1b727220a95);
    }
    hash
}

#[proc_macro_derive(Kowit)]
pub fn kowito_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut generated_fields = Vec::new();

    if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            for field in &fields_named.named {
                let field_ident = field.ident.as_ref().unwrap();
                let field_name_str = field_ident.to_string();
                let field_hash = compute_hash(&field_name_str);
                
                generated_fields.push(quote! {
                    #field_hash => {
                        // Phase 3: Unrolled SIMD bypass logic specializes here per field type!
                        println!("Fast-path zero-decode for field: {} (Hash: {})", stringify!(#field_ident), #field_hash);
                    }
                });
            }
        }
    }

    // Generate the struct implementation with the Schema-JIT methods.
    let expanded = quote! {
        impl #name {
            #[inline(always)]
            pub fn kowito_schema_version() -> &'static str {
                "1.0.0-turbo"
            }
            
            #[inline(always)]
            pub fn process_field_hash(hash: u64) {
                match hash {
                    #(#generated_fields)*
                    _ => {
                        // Skip unknown field quickly without allocation
                    }
                }
            }
        }
    };

    TokenStream::from(expanded)
}
