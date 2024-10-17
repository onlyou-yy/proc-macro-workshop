use proc_macro::{TokenStream};
use quote::{ToTokens};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let _ = input;

    // 将 TokenStream 解析成 syn::Item，因为属性式过程宏几乎可以用在几乎全部地方
    // 派生式过程宏只能用在 struct、Union、enum，所以解析cheng syn::DeriveInput
    // 函数式过程宏可以根据自己的语法规则定义自己的语法树节点，并定义自己的解析规则
    let st = syn::parse_macro_input!(input as syn::Item);

    match do_expand(&st) {
       Ok(token_stream) => token_stream.into(), 
       Err(err) => {
            // 下面这一行拿到的TokenStream是空的，里面只包含了错误信息，没有代码信息
            let mut t = err.to_compile_error();
            // 将原始的用户代码塞进去，这样返回结果中既包含代码信息，也包含错误信息
            t.extend(st.to_token_stream());
            t.into()
       },
    }
}

fn do_expand(st:&syn::Item) -> syn::Result<proc_macro2::TokenStream> {
    // 先判断要处理的是什么类型，如果不能处理就返回错误
    match st {
        syn::Item::Enum(e) => {
            check_enum_order(e)
        },
        _ => {
            // 注意下面call_site()方法的使用，它可以获得过程宏被调用的位置，这样才可以满足测试用例02的输出要求
            //call_site() 获取当前调用 #[sorted] 的位置 
            syn::Result::Err(syn::Error::new(proc_macro2::Span::call_site(), "expected enum or match expression"))
        }
    }
}

fn check_enum_order(st:&syn::ItemEnum) -> syn::Result<proc_macro2::TokenStream> {
    // 构造两个数组，一个是排序的，一个是没有排序的，同时从头遍历，遇到第一个不一致的位置就是需要提示错误的位置
    let origin_order:Vec<_> = st.variants.iter().map(|v| (v.ident.to_string(),v)).collect();
    let mut sorted = origin_order.clone();
    sorted.sort_by(|a,b| a.0.cmp(&b.0));

    for (a,b) in origin_order.iter().zip(sorted) {
        if a.0 != b.0 {
            return syn::Result::Err(syn::Error::new_spanned(&b.1.ident, format!("{} should sort before {}",b.0,a.0)));
        }
    }

    Ok(st.to_token_stream())
}
