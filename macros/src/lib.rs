use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Archetype)]
pub fn derive_archetype_fn(input: TokenStream) -> TokenStream {
    let main_crate = quote!(::entity_data);

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
        impl #generics #main_crate::StaticArchetype for #ident #generics #where_clause {
            const N_COMPONENTS: usize = #fields_len;
        }

        impl #generics #main_crate::ArchetypeState for #ident #generics #where_clause {
            fn ty(&self) -> ::std::any::TypeId {
                ::std::any::TypeId::of::<#ident>()
            }

            fn as_ptr(&self) -> *const u8 {
                self as *const _ as *const u8
            }

            fn forget(self) {
                ::std::mem::forget(self);
            }

            fn metadata(&self) -> fn() -> #main_crate::private::ArchetypeMetadata {
                || #main_crate::private::ArchetypeMetadata {
                    component_type_ids: || #main_crate::private::smallvec![#field_types],
                    component_infos: || #main_crate::private::smallvec![#fields],
                    needs_drop: ::std::mem::needs_drop::<Self>(),
                    drop_func: |p: *mut u8| unsafe { ::std::ptr::drop_in_place(p as *mut Self) },
                }
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }
        }
    }
    .into()
}
