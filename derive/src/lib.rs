use proc_macro2::TokenStream;
use quote::quote;
use serde_derive_internals::{attr::get_serde_meta_items, Ctxt};
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, Lit, Meta, NestedMeta};

/// Parses `#[serde(skip_serializing)]`
fn serde_skipped(cx: &Ctxt, attrs: &[syn::Attribute]) -> bool {
    for meta_items in attrs
        .iter()
        .filter_map(|attr| get_serde_meta_items(cx, attr).ok())
    {
        for meta_item in meta_items {
            match meta_item {
                NestedMeta::Meta(Meta::Path(path))
                    if path
                        .get_ident()
                        .map_or(false, |i| i.to_string() == "skip_serializing") =>
                {
                    return true
                }
                _ => continue,
            }
        }
    }
    false
}

/// Parses `#[serde(rename = "..")]`
fn serde_rename(cx: &Ctxt, field: &syn::Field) -> Option<String> {
    for meta_items in field
        .attrs
        .iter()
        .filter_map(|attr| get_serde_meta_items(cx, attr).ok())
    {
        for meta_item in meta_items {
            match meta_item {
                NestedMeta::Meta(Meta::NameValue(nv))
                    if nv
                        .path
                        .get_ident()
                        .map_or(false, |i| i.to_string() == "rename") =>
                {
                    if let Lit::Str(lit) = nv.lit {
                        return Some(lit.value());
                    }
                }
                _ => continue,
            }
        }
    }
    None
}

fn column_names(data: &DataStruct) -> TokenStream {
    match &data.fields {
        Fields::Named(fields) => {
            let cx = Ctxt::new();
            let column_names_iter = fields
                .named
                .iter()
                .filter(|f| !serde_skipped(&cx, &f.attrs))
                .map(|f| match serde_rename(&cx, f) {
                    Some(name) => name,
                    None => f.ident.as_ref().unwrap().to_string(),
                });

            let tokens = quote! {
                &[#( #column_names_iter,)*]
            };

            // TODO: do something more clever?
            let _ = cx.check();
            tokens
        }
        Fields::Unnamed(_) => {
            quote! { &[] }
        }
        Fields::Unit => panic!("`Row` cannot be derived for unit structs"),
    }
}

// TODO: support wrappers `Wrapper(Inner)` and `Wrapper<T>(T)`.
// TODO: support the `nested` attribute.
// TODO: support the `crate` attribute.
#[proc_macro_derive(Row)]
pub fn row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let column_names = match &input.data {
        Data::Struct(data) => column_names(data),
        Data::Enum(_) | Data::Union(_) => panic!("`Row` can be derived only for structs"),
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // TODO: replace `clickhouse` with `::clickhouse` here.
    let expanded = quote! {
        impl #impl_generics clickhouse::Row for #name #ty_generics #where_clause {
            const COLUMN_NAMES: &'static [&'static str] = #column_names;
        }
    };

    proc_macro::TokenStream::from(expanded)
}
