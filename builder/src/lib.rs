use proc_macro::TokenStream;
use proc_macro2;
use quote::quote;
use syn::{self, spanned::Spanned, LitStr};

#[proc_macro_derive(Builder,attributes(builder))]
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
    let fields_type:syn::Result<Vec<_>> = fields.iter().map(|f| {
        if let Some(inner_type) = get_generic_inner_type(&f.ty, "Option") {
            Ok(quote! {
                std::option::Option<#inner_type>
            })
        } else if get_user_specified_ident_for_vec(&f)?.is_some() {
            let origin_type = &f.ty;
            Ok(quote! {
                #origin_type
            })
        } else {
            let origin_type = &f.ty;
            Ok(quote! {
                std::option::Option<#origin_type>
            })
        }
    }).collect();

    let types = fields_type?;

    // 在生成类型的时候要使用绝对路径避免与当前定义的类型冲突
    // #(重复的内容必须是实现了迭代器的数据)*
    let ret: proc_macro2::TokenStream = quote! {
        #(#fields_ident:#types),*
    };

    Ok(ret.into())
}

fn generate_builder_struct_factory_init_clauses(
    st: &syn::DeriveInput,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    // 也可以像 generate_builder_struct_fields_def 一样生成

    let fields = get_fields_from_derive_input(st)?;

    let init_clauses:syn::Result<Vec<_>> = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            // 这里为什么加个 ? 就要把其他的返回都加上 Ok 包裹？
            // 因为当 get_user_specified_ident_for_vec 报错是就会抛出错误，而错误类型是 Result,
            // map 接收到的返回数据类型就有两种：TokenStream 和 Result，从而出现类型冲突，所以要进行统一化处理
            if get_user_specified_ident_for_vec(&f)?.is_some() {
                Ok(quote! {
                    #ident: std::vec::Vec::new()
                })
            } else {
                Ok(quote! {
                    #ident: std::option::Option::None
                })
            }
        })
        .collect();

    Ok(init_clauses?)
}

fn generate_setter_functions(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fields = get_fields_from_derive_input(st)?;

    let mut final_tokenstream = proc_macro2::TokenStream::new();

    for field in fields {
        let ident = &field.ident;
        let type_ = &field.ty;
        let tokenstream_piece = if let Some(inner_type) = get_generic_inner_type(type_, "Option") {
            quote! {
                fn #ident(&mut self,#ident:#inner_type) -> &mut Self{
                    self.#ident = std::option::Option::Some(#ident);
                    self
                }
            }
        } else if let Some(ref user_specified_ident) = get_user_specified_ident_for_vec(field)? {
            let inner_type = get_generic_inner_type(&field.ty, "Vec").ok_or(syn::Error::new(
                field.span(),
                "each field must be specified with Vec field",
            ))?;
            let mut tokenstream = proc_macro2::TokenStream::new();
            tokenstream.extend(quote! {
                fn #user_specified_ident(&mut self,#user_specified_ident:#inner_type) -> &mut Self{
                    self.#ident.push(#user_specified_ident);
                    self
                }
            });
            if user_specified_ident != ident.as_ref().unwrap() {
                tokenstream.extend(quote! {
                    fn #ident(&mut self,#ident:#type_) -> &mut Self{
                        self.#ident = #ident.clone();
                        self
                    }
                });
            }
            tokenstream
        } else {
            quote! {
                fn #ident(&mut self,#ident:#type_) -> &mut Self{
                    self.#ident = std::option::Option::Some(#ident);
                    self
                }
            }
        };

        final_tokenstream.extend(tokenstream_piece);
    }

    Ok(final_tokenstream)
}

fn generate_build_function(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fields = get_fields_from_derive_input(st)?;
    let mut checker_code_pieces = Vec::new();
    for field in fields {
        let ident = &field.ident;
        let type_ = &field.ty;
        // 只对不是`Option`类型且没有指定each属性的字段生成校验逻辑
        if get_generic_inner_type(type_, "Option").is_none() && get_user_specified_ident_for_vec(field)?.is_none() {
            checker_code_pieces.push(quote! {
                if self.#ident.is_none() {
                    let err = format!("{} field is missing",stringify!(#ident));
                    return std::result::Result::Err(err.into());
                }
            });
        }
    }

    let mut fill_result_clauses = Vec::new();
    for field in fields {
        let ident = &field.ident;
        let type_ = &field.ty;
        // 需要先判断是有自定 each ，再判断是否是 Option，因为 Option比 each 范围更广
        if get_user_specified_ident_for_vec(field)?.is_some() {
            fill_result_clauses.push(quote! {
                #ident:self.#ident.clone()
            });
        } else if get_generic_inner_type(type_, "Option").is_none() {
            fill_result_clauses.push(quote! {
                #ident:self.#ident.clone().unwrap()
            });
        } else {
            fill_result_clauses.push(quote! {
                #ident:self.#ident.clone()
            });
        }
    }

    let struct_name_ident = &st.ident;
    let token_stream = quote! {
        pub fn build(&mut self) -> std::result::Result<#struct_name_ident,std::boxed::Box<dyn std::error::Error>>{
            #(#checker_code_pieces)*

            let ret = #struct_name_ident {
                #(#fill_result_clauses,)*
            };

            return std::result::Result::Ok(ret);
        }
    };

    Ok(token_stream)
}

fn get_generic_inner_type<'a>(
    t: &'a syn::Type,
    outer_ident_name: &'a str,
) -> Option<&'a syn::Type> {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = t
    {
        // 有可能是是多种写法的 Option<T>,std::option::Option<T>,所以要去最后一项
        if let Some(seg) = segments.last() {
            if seg.ident == outer_ident_name {
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                    args,
                    ..
                }) = &seg.arguments
                {
                    // 范型也可以有多个，取出第一个即可
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.first() {
                        return Some(inner_type);
                    }
                }
            }
        }
    }
    return None;
}

fn get_user_specified_ident_for_vec(field: &syn::Field) -> syn::Result<Option<syn::Ident>> {
    for attr in &field.attrs {
        if attr.path().is_ident("builder") {
            let mut ident = None;
            let _ret = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("each") {
                    let value = meta.value()?;
                    let s: LitStr = value.parse()?;
                    ident = Some(syn::Ident::new(s.value().as_str(), attr.span()));
                    ()
                } else {
                    if let syn::Meta::List(ref list) = attr.meta {
                        eprintln!("metalist,{list:#?}");
                        return Err(syn::Error::new_spanned(list, r#"expected `builder(each = "...")`"#));
                    }
                }
                Ok(())
            });
            return Ok(ident);
        }
    }
    Ok(None)
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
    // 如果代码比较复杂，可以在根目录下运行cargo expand将生成结果复制到编辑器获取play.rust-lang.org中再查看错误
    let builder_name_ident = syn::Ident::new(&builder_name, st.span());

    let builder_struct_fields_def = generate_builder_struct_fields_def(st)?;
    let builder_struct_factory_init_clauses = generate_builder_struct_factory_init_clauses(st)?;
    let setter_functions = generate_setter_functions(st)?;
    let build_function = generate_build_function(st)?;

    // 使用 quote! 插入并生成新的 proc_macro2::TokenStream
    let ret = quote! {
        pub struct #builder_name_ident {
            #builder_struct_fields_def
        }

        impl #builder_name_ident{
            #setter_functions

            #build_function
        }

        impl #struct_name_ident {
            pub fn builder() -> #builder_name_ident {
                #builder_name_ident {
                    #(#builder_struct_factory_init_clauses),*
                }
            }
        }

    };

    Ok(ret)
}
