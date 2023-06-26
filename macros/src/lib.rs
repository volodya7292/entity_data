use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Implements archetype capabilities for `struct`.
#[proc_macro_derive(Archetype)]
pub fn derive_archetype_fn(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
                    #main_crate::private::offset_of!(Self, #field_ident)
                }
            } else {
                let i = syn::Index::from(i);
                quote! {
                    #main_crate::private::offset_of!(Self, #i)
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
                },
            }
        })
        .collect();

    let fields_len = field_impls.len();

    // Check component uniqueness
    {
        let mut field_names: Vec<_> = types.iter().map(|v| v.to_string()).collect();
        field_names.sort();
        let initial_len = field_names.len();

        field_names.dedup();
        let deduped_len = field_names.len();

        if initial_len != deduped_len {
            panic!("Archetype contains multiple components of the same type.");
        }
    }

    let mut field_types = TokenStream::new();
    field_types.extend(types.into_iter());

    let mut fields = TokenStream::new();
    fields.extend(field_impls.into_iter());

    quote! {
        impl #generics #main_crate::StaticArchetype for #ident #generics #where_clause {
            const N_COMPONENTS: usize = #fields_len;

            fn metadata() -> #main_crate::private::ArchetypeMetadata {
                #main_crate::private::ArchetypeMetadata {
                    type_id: ::std::any::TypeId::of::<Self>(),
                    component_type_ids: || #main_crate::private::smallvec![#field_types],
                    component_infos: || #main_crate::private::smallvec![#fields],
                    size: ::std::mem::size_of::<Self>(),
                    needs_drop: ::std::mem::needs_drop::<Self>(),
                    drop_fn: |p: *mut u8| unsafe { ::std::ptr::drop_in_place(p as *mut Self) },
                }
            }
        }

        impl #generics #main_crate::ArchetypeState for #ident #generics #where_clause {
            fn ty(&self) -> ::std::any::TypeId {
                ::std::any::TypeId::of::<Self>()
            }

            fn as_ptr(&self) -> *const u8 {
                self as *const _ as *const u8
            }

            fn forget(self) {
                ::std::mem::forget(self);
            }

            fn metadata(&self) -> #main_crate::private::ArchetypeMetadata {
                <Self as #main_crate::StaticArchetype>::metadata()
            }

            fn num_components(&self) -> usize {
                #fields_len
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
