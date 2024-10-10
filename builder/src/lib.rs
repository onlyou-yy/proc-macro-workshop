use proc_macro::TokenStream;
use proc_macro2;
use quote::quote;
use syn::{self, spanned::Spanned};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let _ = input;
    let st = syn::parse_macro_input!(input as syn::DeriveInput);
    // eprintln!("{input_ast:#?}");

    match do_expand(&st) {
        Ok(token_stream) => token_stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

type StructFields = syn::punctuated::Punctuated<syn::Field, syn::Token![,]>;

fn get_fields_from_derive_input(st: &syn::DeriveInput) -> syn::Result<&StructFields> {
    // 从 DeriveInput 中的 Data 解析出字段名字
    if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = st.data
    {
        return Ok(named);
    };
    Err(syn::Error::new_spanned(st, "解析字段名错误"))
}

fn generate_builder_struct_fields_def(
    st: &syn::DeriveInput,
) -> syn::Result<proc_macro2::TokenStream> {
    let fields = get_fields_from_derive_input(st)?;

    let fields_ident = fields.iter().map(|f| &f.ident);
    let fields_type = fields.iter().map(|f| &f.ty);

    // 在生成类型的时候要使用绝对路径避免与当前定义的类型冲突
    // #(重复的内容必须是实现了迭代器的数据)*
    let ret: proc_macro2::TokenStream = quote! {
        #(#fields_ident:std::option::Option<#fields_type>),*
    };

    Ok(ret.into())
}

fn generate_builder_struct_factory_init_clauses(
    st: &syn::DeriveInput,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    // 也可以像 generate_builder_struct_fields_def 一样生成

    let fields = get_fields_from_derive_input(st)?;

    let init_clauses = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! {
                #ident: std::option::Option::None
            }
        })
        .collect();

    Ok(init_clauses)
}

fn generate_setter_functions(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fields = get_fields_from_derive_input(st)?;

    let idents: Vec<_> = fields.iter().map(|f| &f.ident).collect();
    let types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    let mut final_tokenstream = proc_macro2::TokenStream::new();

    for (ident, type_) in idents.iter().zip(types.iter()) {
        let tokenstream_piece = quote! {
            fn #ident(&mut self,#ident:#type_) -> &mut Self{
                self.#ident = std::option::Option::Some(#ident);
                self
            }
        };

        final_tokenstream.extend(tokenstream_piece);
    }

    Ok(final_tokenstream)
}

fn do_expand(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    // 获取到结构体的名字 ident;
    let struct_name_ident = st.ident.clone();
    // 获取到结构体的名字
    let struct_name_literal = struct_name_ident.to_string();
    // 创建 builder 结构体的名字
    let builder_name = format!("{struct_name_literal}Builder");
    // 创建 builder 结构体名字的 ident，new的第二个参数是 span，用于定位新增的代码是在哪个位置
    // 方便之后报错定位错误，这里使用 st.span() ,报错时就会直接提示是修饰的结构的的位置的错误
    let builder_name_ident = syn::Ident::new(&builder_name, st.span());

    let builder_struct_fields_def = generate_builder_struct_fields_def(st)?;
    let builder_struct_factory_init_clauses = generate_builder_struct_factory_init_clauses(st)?;
    let setter_functions = generate_setter_functions(st)?;

    // 使用 quote! 插入并生成新的 proc_macro2::TokenStream
    let ret = quote! {
        pub struct #builder_name_ident {
            #builder_struct_fields_def
        }

        impl #struct_name_ident {
            pub fn builder() -> #builder_name_ident {
                #builder_name_ident {
                    #(#builder_struct_factory_init_clauses),*
                }
            }
        }

        impl #builder_name_ident{
            #setter_functions
        }
    };

    Ok(ret)
}
