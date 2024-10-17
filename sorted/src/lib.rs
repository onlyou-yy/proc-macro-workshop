use proc_macro::{TokenStream};
use quote::{ToTokens};
use syn::visit_mut::{self, VisitMut};

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

#[proc_macro_attribute]
pub fn check(_: TokenStream, input: TokenStream) -> TokenStream {
    let mut st = syn::parse_macro_input!(input as syn::ItemFn);

    match do_match_expand(&mut st) {
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

fn do_match_expand(st:&mut syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    // 创建Visitor并通过Visitor模式完成核心工作：从语法树节点找到满足条件的match语句块
    let mut visitor = MatchVisitor{err:None};

    visitor.visit_item_fn_mut(st);

    if visitor.err.is_none() {
        return syn::Result::Ok( st.to_token_stream())
    } else {
        return syn::Result::Err(visitor.err.unwrap());
    }
}


struct MatchVisitor {
    err: Option<syn::Error>
}

impl syn::visit_mut::VisitMut for MatchVisitor {
    fn visit_expr_match_mut(&mut self, i: &mut syn::ExprMatch) {
        let mut target_idx: isize = -1;
        for (idx,attr) in i.attrs.iter().enumerate() {
            if attr.path().is_ident("sorted") {
                target_idx = idx as isize;
                break;
            }
        }

        if target_idx == -1 {
            visit_mut::visit_expr_match_mut(self, i);
            return;
        }

        // 删除掉编译器不支持的写在match语句块上面的属性标签
        i.attrs.remove(target_idx as usize);

        let mut match_arm_names = Vec::new();

        for arm in &i.arms {
            // 要处理三种匹配模式，测试用例07告诉我们对于不支持的模式需要抛出异常
            match &arm.pat {
                syn::Pat::Path(p) => {
                    match_arm_names.push((get_path_string(&p.path),&p.path))
                },
                syn::Pat::TupleStruct(p) => {
                    match_arm_names.push((get_path_string(&p.path),&p.path))
                },
                syn::Pat::Struct(p) => {
                    match_arm_names.push((get_path_string(&p.path),&p.path))
                },
                _ => {
                    self.err = Some(syn::Error::new_spanned(&arm.pat, "unsupported by #[sorted]"));
                    return;
                },
            }
        }

        let mut sorted_names = match_arm_names.clone();
        sorted_names.sort_by(|a,b|{a.0.cmp(&b.0)});
        for (a,b) in match_arm_names.iter().zip(sorted_names) {
            if a.0 != b.0 {
                self.err = Some(syn::Error::new_spanned(b.1, format!("{} should sort before {}", b.0, a.0)));
                return;
            }
        }

        // 继续迭代深层次的match
        visit_mut::visit_expr_match_mut(self, i)
    }
}

fn get_path_string(p:&syn::Path) -> String {
    let mut buf = Vec::new();
    for s in &p.segments {
        buf.push(s.ident.to_string());
    }
    return buf.join("::");
}