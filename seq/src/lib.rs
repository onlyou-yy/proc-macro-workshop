use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let _ = input;
    let st = parse_macro_input!(input as SeqParser);

    return  proc_macro2::TokenStream::new().into();
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
