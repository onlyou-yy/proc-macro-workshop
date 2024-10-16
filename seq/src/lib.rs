use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let st = parse_macro_input!(input as SeqParser);

    // 下面5行是第三关新加入的
    let mut ret = proc_macro2::TokenStream::new();
    for i in st.start..st.end {
        ret.extend(st.expand(&st.body, i))
    }

    return ret.into();
}

// 定义解析自己语法，首先需要定义自己的语法树节点
struct SeqParser {
    variable_ident: syn::Ident,
    start: isize,
    end: isize,
    body: proc_macro2::TokenStream,
}

// SeqParser 实现 syn::parse::Parse 的trait,从而提供将TokenStream 解析成 ast 的能力
impl syn::parse::Parse for SeqParser {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // 我们要解析形如 `N in 0..512 {.......}` 这样的代码片段
        // 假定`ParseStream`当前游标对应的是一个可以解析为`Ident`类型的Token，
        // 如果真的是`Ident`类型节点，则返回Ok并将当前读取游标向后移动一个Token
        // 如果不是`Ident`类型，则返回Err,说明语法错误，直接返回
        let variable_ident:syn::Ident = input.parse()?;

        // 假定`ParseStream`当前游标对应的是一个写作`in`的自定义的Token
        input.parse::<syn::Token![in]>()?;

        // 假定`ParseStream`当前游标对应的是一个可以解析为整形数字面量的Token，
        let start_lit:syn::LitInt = input.parse()?;

        // 假定`ParseStream`当前游标对应的是一个写作`..`的自定义的Token
        input.parse::<syn::Token![..]>()?;

        // 假定`ParseStream`当前游标对应的是一个可以解析为整形数字面量的Token，
        let end_lit:syn::LitInt = input.parse()?;

        // 这里展示了braced!宏的用法，用于把一个代码块整体读取出来，如果读取成功就将代码块
        // 内部数据作为一个`ParseBuffer`类型的数据返回，同时把读取游标移动到整个代码块的后面
        let body_buf;
        syn::braced!(body_buf in input);
        let body:proc_macro2::TokenStream = body_buf.parse()?;

        let t = SeqParser{
            variable_ident,
            start:start_lit.base10_parse()?,
            end:end_lit.base10_parse()?,
            body
        };

        Ok(t)
    }
}


impl SeqParser {
    fn expand(&self, ts: &proc_macro2::TokenStream, n: isize) -> proc_macro2::TokenStream {
        let buf = ts.clone().into_iter().collect::<Vec<_>>();
        let mut ret = proc_macro2::TokenStream::new();
        
        // 这里为了简单，使用了for循环，实际我想表达的意思是，
        // 这个idx你可以随心所欲的控制，跳着访问，回溯已经访问过的节点等等，都可以
        for idx in 0..buf.len() {
            let tree_node = &buf[idx];
            match tree_node {
                proc_macro2::TokenTree::Group(g) => {
                    // 如果是括号包含的内容，我们就要递归处理内部的TokenStream
                    let new_stream = self.expand(&g.stream(), n);
                    // 这里需要注意，上一行中g.stream()返回的是Group内部的TokenStream，
                    // 也就是说不包含括号本身，所以要在下面重新套上一层括号，而且括号的
                    // 种类要与原来保持一致。 
                    let mut wrap_in_group = proc_macro2::Group::new(g.delimiter(), new_stream);
                    wrap_in_group.set_span(g.span());
                    ret.extend(quote::quote!(#wrap_in_group));
                }
                proc_macro2::TokenTree::Ident(i) => {
                    // 如果是一个Ident，那么看一下是否为要替换的变量标识符，如果是则替换，
                    // 如果不是则透传。
                    if i == &self.variable_ident {
                        // 注意第二关的测试用例中，过程宏期待的是一个Literal，所以为了
                        // 通过，我们也要产生一个Literal
                        let new_ident = proc_macro2::Literal::i64_unsuffixed(n as i64);
                        ret.extend(quote::quote!(#new_ident));
                    } else {
                        ret.extend(quote::quote!(#tree_node));
                    }
                }
                _ => {
                    // 对于其它的元素（也就是Punct和Literal），原封不动透传
                    ret.extend(quote::quote!(#tree_node));
                }
            }
        }
        ret
    }
}