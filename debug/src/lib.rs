use std::collections::HashMap;

use proc_macro::{TokenStream};
use quote::quote;
use syn::{parse_macro_input, parse_quote, visit::{self, Visit}, DeriveInput, LitStr};

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
    let mut generic_param_to_modify = st.generics.clone();

    if let Some(hatch) = get_struct_escape_hatch(st) {
        generic_param_to_modify.make_where_clause();
        generic_param_to_modify
                    .where_clause
                    .as_mut()
                    .unwrap()
                    .predicates
                    .push(syn::parse_str(hatch.as_str()).unwrap());
    }else {
        let fields = get_fields_from_derive_input(st)?;
        let mut field_type_names:Vec<String> = Vec::new();
        let mut phantomdata_type_param_names:Vec<String> = Vec::new();
        for field in fields {
            if let Some(s) = get_field_type_name(field)? {
                field_type_names.push(s);
            }
            if let Some(s) = get_phantomdata_generic_type_name(field)?{
                phantomdata_type_param_names.push(s);
            }
        }
        
        //找到关联类型信息
        let associated_types_map = get_generic_associated_types(st);
        // 取出范型定义，然后为每个范型追加 Debug 约束，之后重新插入到语法树中
        for g in generic_param_to_modify.params.iter_mut() {
            if let syn::GenericParam::Type(t) = g{
                let type_param_name = t.ident.to_string();
                // 如果是PhantomData，就不要对泛型参数`T`本身再添加约束了,除非`T`本身也被直接使用了
                if phantomdata_type_param_names.contains(&type_param_name) && !field_type_names.contains(&type_param_name) {
                    continue;
                }

                // 如果是关联类型，就不要对泛型参数`T`本身再添加约束了,除非`T`本身也被直接使用了
                if associated_types_map.contains_key(&type_param_name) && !field_type_names.contains(&type_param_name) {
                    continue;
                }

                // parse_quote! 将数据解析为语法树节点
                t.bounds.push(parse_quote!(std::fmt::Debug));
            }
        }

        // 关联类型的约束要放到where子句里
        generic_param_to_modify.make_where_clause();
        for (_, associated_types) in associated_types_map {
            for associated_type in associated_types {
                generic_param_to_modify.where_clause.as_mut().unwrap().predicates.push(parse_quote!(#associated_type:std::fmt::Debug));
            }
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

fn get_field_type_name(field:&syn::Field) -> syn::Result<Option<String>> {
    if let syn::Type::Path(syn::TypePath{path: syn::Path{ref segments, ..}, ..}) = field.ty {
        if let Some(syn::PathSegment{ref ident,..}) = segments.last() {
            return Ok(Some(ident.to_string()))
        }
    }
    return Ok(None)
}

fn get_phantomdata_generic_type_name(field:&syn::Field) -> syn::Result<Option<String>>{
    if let syn::Type::Path(syn::TypePath{path: syn::Path{ref segments, ..}, ..}) = field.ty {
        if let Some(syn::PathSegment{ref ident, ref arguments}) = segments.last() {
            if ident == "PhantomData" {
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments{args, ..}) = arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::Path( ref gp))) = args.first() {
                        if let Some(generic_ident) = gp.path.segments.first() {
                            return Ok(Some(generic_ident.ident.to_string()))
                        }
                    }
                }
            }
        }
    }
    return Ok(None)
}

fn get_generic_associated_types(st:&syn::DeriveInput) -> HashMap<String,Vec<syn::TypePath>> {
    // 构建筛选条件
    let origin_generic_param_names = st.generics.params.iter().filter_map(|f| {
        if let syn::GenericParam::Type(ty) = f {
            return  Some(ty.ident.to_string());
        }
        return None;
    }).collect();

    let mut visitor = TypePathVisitor {
        generic_type_names:origin_generic_param_names,
        associated_types:HashMap::new()
    };

    visitor.visit_derive_input(st);
    return  visitor.associated_types;

}

fn get_struct_escape_hatch(st: &syn::DeriveInput) -> Option<String> {
    if let Some(inert_attr) = st.attrs.last() {
        if inert_attr.path().is_ident("debug") {
            let mut lit= None;
            let _ret = inert_attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("bound") {
                    let value = meta.value()?;
                    let s:LitStr = value.parse()?;
                    lit = Some(s.value());
                }
                Ok(())
            });
            return lit;
        }
    }
    None
}

fn do_expand(st:&syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let debug_trait_tokenstream = generate_debug_trait(st)?;
    return Ok(debug_trait_tokenstream);
}


// 使用 syn 的 visit 模式可以实现当遍历语法树到指定的节点的时候触发回调
// 需要在 features 中开启 visit 才可以使用
struct TypePathVisitor {
    // 这个是筛选条件，里面记录了所有的泛型参数的名字，例如`T`,`U`等
    generic_type_names:Vec<String>,
    // 这里记录了所有满足条件的语法树节点
    associated_types: HashMap<String, Vec<syn::TypePath>>,  
}

impl<'ast> Visit<'ast> for TypePathVisitor {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        // 只对 T::Value1 这种类型进行处理
        if node.path.segments.len() >= 2 {
            let generic_type_name = node.path.segments[0].ident.to_string();
            if self.generic_type_names.contains(&generic_type_name) {
                self.associated_types.entry(generic_type_name).or_insert(Vec::new()).push(node.clone());
            }
        }
        // Visit 模式要求在当前节点访问完成后，继续调用默认实现的visit方法，从而遍历到所有的
        // 必须调用这个函数，否则遍历到这个节点就不再往更深层走了
        visit::visit_type_path(self, node);
    }
}