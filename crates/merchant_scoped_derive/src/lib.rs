//! `#[derive(MerchantScoped)]`: implements `crate::auth::MerchantScoped` for a request type from
//! the fields marked `#[merchant]`.
//!
//! ```ignore
//! #[derive(Deserialize, MerchantScoped)]
//! pub struct ActivateRoutingConfigRequest {
//!     #[merchant]
//!     pub created_by: String,
//!     pub routing_algorithm_id: String,
//! }
//! ```
//!
//! A marked field is anything that is itself `MerchantScoped`: a `String` or `MerchantId` naming
//! one merchant, an `Option` of one, or a nested request type that derives it too. The generated
//! `merchant_ids` lists every marked field's merchants in declaration order.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields};

#[proc_macro_derive(MerchantScoped, attributes(merchant))]
pub fn derive_merchant_scoped(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "MerchantScoped can only be derived for a struct with named fields",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            data.fields.span(),
            "MerchantScoped can only be derived for a struct with named fields",
        ));
    };

    let mut marked = Vec::new();
    for field in &fields.named {
        let mut attrs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("merchant"));
        if let Some(attr) = attrs.next() {
            attr.meta.require_path_only()?;
            if let Some(duplicate) = attrs.next() {
                return Err(syn::Error::new(duplicate.span(), "duplicate #[merchant]"));
            }
            marked.push(field.ident.as_ref());
        }
    }
    if marked.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "MerchantScoped needs at least one field marked #[merchant]: the field(s) naming the \
             merchant this request acts on",
        ));
    }

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics crate::auth::MerchantScoped for #name #ty_generics #where_clause {
            fn merchant_ids(&self) -> ::std::vec::Vec<&str> {
                let mut merchant_ids = ::std::vec::Vec::new();
                #(
                    merchant_ids.extend(crate::auth::MerchantScoped::merchant_ids(&self.#marked));
                )*
                merchant_ids
            }
        }
    })
}
