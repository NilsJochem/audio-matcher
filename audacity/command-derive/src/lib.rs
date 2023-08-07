use darling::FromField;
use proc_macro::{self, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::{parse_macro_input, DeriveInput};

#[derive(FromField)]
#[darling(attributes(my_trait))]
struct Opts {
    name: Option<String>,
}

#[proc_macro_derive(ToString)]
pub fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    let match_variants = match data {
        syn::Data::Struct(_data) => unimplemented!(),
        syn::Data::Union(_data) => unimplemented!(),
        syn::Data::Enum(data) => match_enum(data),
    };
    let output = quote! {
        impl ToString for #ident {
            fn to_string(&self) -> String {
                let mut s = String::new();
                match self {
                    #match_variants
                };
                s
            }
        }
    };
    output.into()
}

fn match_enum(data: syn::DataEnum) -> proc_macro2::TokenStream {
    let mut tokens = quote!();
    for variant in data.variants {
        tokens.append_all(match_enum_variant(variant));
    }
    tokens
}

fn match_enum_variant(variant: syn::Variant) -> proc_macro2::TokenStream {
    let mut sub_tokens = quote!();
    for field in &variant.fields {
        sub_tokens.append_all(match_field(field))
    }
    let variant_name = variant.ident.to_string();
    let variant_ident = &variant.ident;
    let fields = variant.fields.iter().map(|f| f.ident.as_ref().unwrap());
    quote! {
        #variant_ident{#(#fields),*} => {
            s.push_str(#variant_name);
            s.push(':');
            #sub_tokens
        },
    }
}
fn match_field(field: &syn::Field) -> proc_macro2::TokenStream {
    let ident = field.ident.as_ref().expect("no Tuple structs");
    let name = Opts::from_field(field)
        .expect("wrong Options")
        .name
        .unwrap_or_else(|| {
            let name_full = ident.to_string();
            let name_trunc = name_full.trim_start_matches('_');
            let mut name = name_trunc[..1].to_ascii_uppercase();
            name.push_str(&name_trunc[1..].to_ascii_lowercase());
            name
        }); // TODO find out how darling works

    match extract_type_from_option(&field.ty) {
        Some(_) => quote! {
            push_if_some(&mut s, #name, #ident);
        },
        None => quote! {
            push(&mut s, #name, #ident);
        },
    }
}

fn extract_type_from_option(ty: &syn::Type) -> Option<&syn::Type> {
    // If it is not `TypePath`, it is not possible to be `Option<T>`, return `None`
    if let syn::Type::Path(syn::TypePath { qself: None, path }) = ty {
        // We have limited the 5 ways to write `Option`, and we can see that after `Option`,
        // there will be no `PathSegment` of the same level
        // Therefore, we only need to take out the highest level `PathSegment` and splice it into a string
        // for comparison with the analysis result
        let segments_str = &path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join(":");
        // Concatenate `PathSegment` into a string, compare and take out the `PathSegment` where `Option` is located
        let option_segment = ["Option", "std:option:Option", "core:option:Option"]
            .iter()
            .find(|s| segments_str == *s)
            .and_then(|_| path.segments.last());
        let inner_type = option_segment
            // Take out the generic parameters of the `PathSegment` where `Option` is located
            // If it is not generic, it is not possible to be `Option<T>`, return `None`
            // But this situation may not occur
            .and_then(|path_seg| match &path_seg.arguments {
                syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                    args,
                    ..
                }) => args.first(),
                _ => None,
            })
            // Take out the type information in the generic parameter
            // If it is not a type, it is not possible to be `Option<T>`, return `None`
            // But this situation may not occur
            .and_then(|generic_arg| match generic_arg {
                syn::GenericArgument::Type(ty) => Some(ty),
                _ => None,
            });
        // Return `T` in `Option<T>`
        return inner_type;
    }
    None
}
