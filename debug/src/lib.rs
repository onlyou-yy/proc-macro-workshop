use proc_macro::{TokenStream};
use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput};

#[proc_macro_derive(CustomDebug,attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let st = parse_macro_input!(input as DeriveInput);
    match do_expand(&st) {
        Ok(token_stream) => token_stream.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

type StructFields = syn::punctuated::Punctuated<syn::Field, syn::Token![,]>;
fn get_fields_from_derive_input(st:&syn::DeriveInput) -> syn::Result<&StructFields> {
    if let syn::Data::Struct(syn::DataStruct{
        fields:syn::Fields::Named(syn::FieldsNamed{
            named,..
        }),..
    }) = &st.data {
        return Ok(named);
    }
    Err(syn::Error::new_spanned(st, "字段解析错误"))
}

fn generate_debug_fmt_body(st:&syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name_ident = &st.ident;
    let struct_name_literal = struct_name_ident.to_string();
    let fields = get_fields_from_derive_input(st)?;

    let mut fmt_body_stream = proc_macro2::TokenStream::new();
    fmt_body_stream.extend(quote! {
        fmt.debug_struct(#struct_name_literal)
    });

    for field in fields {
        let ident = field.ident.as_ref().unwrap();
        let ident_literal = ident.to_string();

        let mut format_str = "{:?}".to_string();
        if let Some(format) = get_custom_format_of_fields(field)? {
            format_str = format;
        }
        fmt_body_stream.extend(quote! {
            .field(#ident_literal,&format_args!(#format_str,&self.#ident))
        });
    }

    fmt_body_stream.extend(quote! {
        .finish()
    });
    Ok(fmt_body_stream)
}

fn generate_debug_trait(st:&syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream>{
    let struct_name_ident = &st.ident;

    let fmt_body_stream = generate_debug_fmt_body(st)?;
    
    // 取出范型定义，然后为每个范型追加 Debug 约束，之后重新插入到语法树中
    let mut generic_param_to_modify = st.generics.clone();
    for g in generic_param_to_modify.params.iter_mut() {
        if let syn::GenericParam::Type(t) = g{
            // parse_quote! 将数据解析为语法树节点
            t.bounds.push(parse_quote!(std::fmt::Debug));
        }
    }

    // 使用工具函数 split_for_impl 将范型参数提取成三个片段，分别为 impl,type,where
    let (impl_generic,type_generic,where_generic) = generic_param_to_modify.split_for_impl();

    let ret = quote! {
        impl #impl_generic std::fmt::Debug for #struct_name_ident #type_generic #where_generic {
            fn fmt(&self,fmt:&mut std::fmt::Formatter) -> std::fmt::Result {
                #fmt_body_stream
            }
        }
    };
    return Ok(ret);
}

fn get_custom_format_of_fields(field:&syn::Field) -> syn::Result<Option<String>> {
    for attr in &field.attrs{
        if let syn::Meta::NameValue(syn::MetaNameValue{
            ref path,
            ref value,
            ..
        }) = attr.meta {
            if path.is_ident("debug") {
                if let syn::Expr::Lit(syn::ExprLit{lit:syn::Lit::Str(ident_str),..}) = value {
                    return Ok(Some(ident_str.value()));
                }
            }
        }
    }
    Ok(None)
}

fn do_expand(st:&syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let debug_trait_tokenstream = generate_debug_trait(st)?;
    return Ok(debug_trait_tokenstream);
}
