use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{parse_macro_input, DeriveInput, Token, Type};

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
                    needs_drop: ::std::mem::needs_drop::<#field_ty>(),
                    drop_func: |p: *mut u8| unsafe { ::std::ptr::drop_in_place(p as *mut #field_ty) }
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

            fn metadata() -> fn() -> #main_crate::private::ArchetypeMetadata {
                || #main_crate::private::ArchetypeMetadata {
                    component_type_ids: || #main_crate::private::smallvec![#field_types],
                    component_infos: || #main_crate::private::smallvec![#fields],
                    needs_drop: ::std::mem::needs_drop::<Self>(),
                    drop_func: |p: *mut u8| unsafe { ::std::ptr::drop_in_place(p as *mut Self) },
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

            fn metadata(&self) -> fn() -> #main_crate::private::ArchetypeMetadata {
                <Self as #main_crate::StaticArchetype>::metadata()
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

struct ComponentInfo {
    mutable: Option<Token![mut]>,
    ty: Type,
}

impl Parse for ComponentInfo {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ComponentInfo {
            mutable: input.parse()?,
            ty: input.parse()?,
        })
    }
}

struct IterSetInfo {
    access_ident: Ident,
    _comma: Token![,],
    components: Punctuated<ComponentInfo, Comma>,
}

impl Parse for IterSetInfo {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(IterSetInfo {
            access_ident: input.parse()?,
            _comma: input.parse()?,
            components: input.parse_terminated(ComponentInfo::parse)?,
        })
    }
}

/// Helps to iterate entities with intersecting components.
///
/// # Example:
///
/// ```
/// let access: SystemAccess = entity_storage.access();
///
/// for v in crate::iter_set!(access, Comp1, mut Comp2) {
///     let (comp1, comp2): (&Comp1, &mut Comp2) = v;
///     println!("{}", comp1.some_field);
/// }
/// ```
#[proc_macro]
pub fn iter_set(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let main_crate = quote!(::entity_data);

    let IterSetInfo {
        access_ident,
        components,
        ..
    } = parse_macro_input!(input as IterSetInfo);

    let b_comp_set_init: TokenStream = components
        .iter()
        .map(|v| {
            let ComponentInfo { mutable, ty } = v;
            if mutable.is_some() {
                quote!(b_comp_set = b_comp_set.with_mut::<#ty>();)
            } else {
                quote!(b_comp_set = b_comp_set.with::<#ty>();)
            }
        })
        .flatten()
        .collect();

    let comp_storages: TokenStream = components
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let ComponentInfo { ty, mutable } = v;

            let storage_name = Ident::new(&format!("storage{}", i), Span::call_site());

            if mutable.is_some() {
                quote!(let mut #storage_name = #access_ident.component_mut::<#ty>();)
            } else {
                quote!(let #storage_name = #access_ident.component::<#ty>();)
            }
        })
        .flatten()
        .collect();

    let comp_values: TokenStream = components
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let ComponentInfo {  mutable, .. } = v;

            let storage_name = Ident::new(&format!("storage{}", i), Span::call_site());
            let comp_name = Ident::new(&format!("comp{}", i), Span::call_site());

            if mutable.is_some() {
                quote!(let #comp_name = unsafe { #storage_name.get_mut(&entity).unwrap_unchecked() };)
            } else {
                quote!(let #comp_name = unsafe { #storage_name.get(&entity).unwrap_unchecked() };)
            }
        })
        .flatten()
        .collect();

    let return_values: TokenStream = (0..components.len())
        .map(|i| {
            let comp_name = Ident::new(&format!("comp{}", i), Span::call_site());
            quote!(#comp_name, )
        })
        .flatten()
        .collect();

    quote! {{
        let mut b_comp_set = #main_crate::system::BCompSet::new();

        #b_comp_set_init

        let set = #access_ident.component_set(&b_comp_set);
        let entity_iter = set.into_entities_iter();

        #comp_storages

        entity_iter.map(move |entity| {
            #comp_values

            (#return_values)
        })
    }}
    .into()
}
