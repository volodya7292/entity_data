use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Archetype)]
pub fn derive_archetype_fn(input: TokenStream) -> TokenStream {
    let main_crate = if std::env::var("CARGO_PKG_NAME").unwrap() == "entity_data" {
        quote!(::entity_data)
    } else {
        quote!(crate)
    };

    let DeriveInput {
        ident,
        data,
        generics,
        ..
    } = parse_macro_input!(input as DeriveInput);

    let where_clause = &generics.where_clause;

    let fields = if let syn::Data::Struct(data) = data {
        data.fields
    } else {
        panic!("Not a structure!");
    };

    let types: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_ty = &field.ty;
            quote! {
                ::std::any::TypeId::of::<#field_ty>(),
            }
        })
        .collect();

    let field_impls: Vec<_> = fields
        .into_iter().enumerate()
        .map(|(i, field)| {
            let field_ty = field.ty;

            let offset = if let Some(field_ident) = &field.ident {
                quote! {
                    #main_crate::private::offset_of!(#ident, #field_ident)
                }
            } else {
                let i = syn::Index::from(i);
                quote! {
                    #main_crate::private::offset_of!(#ident, #i)
                }
            };

            quote! {
                #main_crate::private::ComponentInfo {
                    type_id: ::std::any::TypeId::of::<#field_ty>(),
                    range: {
                        let offset = #offset;
                        let size = ::std::mem::size_of::<#field_ty>();
                        offset..(offset + size)
                    },
                    needs_drop: ::std::mem::needs_drop::<#field_ty>(),
                    drop_func: |p: *mut u8| unsafe { ::std::ptr::drop_in_place(p as *mut #field_ty) }
                },
            }
        })
        .collect();

    let fields_len = field_impls.len();

    let mut field_types = proc_macro2::TokenStream::new();
    field_types.extend(types.into_iter());

    let mut fields = proc_macro2::TokenStream::new();
    fields.extend(field_impls.into_iter());

    quote! {

        impl #generics #main_crate::IsArchetype for #ident #generics #where_clause {}

        impl #generics #main_crate::ArchetypeImpl<#fields_len> for #ident #generics #where_clause {
            fn component_type_ids() -> [::std::any::TypeId; #fields_len] {
                [#field_types]
            }

            fn component_infos() -> [#main_crate::private::ComponentInfo; #fields_len] {
                [#fields]
            }
        }
    }
    .into()
}
