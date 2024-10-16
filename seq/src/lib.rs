use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let st = parse_macro_input!(input as SeqParser);

    let mut ret = proc_macro2::TokenStream::new();

    // 从TokenStream创建TokenBuffer
    let buffer = syn::buffer::TokenBuffer::new2(st.body.clone());

    // 首先尝试寻找`#(xxxxxxxxx)*`模式的代码块
    let(ret_1, expanded) =  st.find_block_to_expand_and_do_expand(buffer.begin());
    if expanded {
        return ret_1.into()
    }

    // 走到这里，说明`#(xxxxxxxxx)*`这个模式没有匹配到，那么重新使用上一关的方式，在整个代码块中尝试展开
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
        let mut idx = 0;
        while idx < buf.len() {
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
                proc_macro2::TokenTree::Ident(prefix) => {
                    if idx + 2 < buf.len() { // 我们需要向后预读两个TokenTree元素
                        if let proc_macro2::TokenTree::Punct(p) = &buf[idx + 1] {
                            // 井号是一个比较少见的符号，
                            // 我们尽量早一些判断井号是否存在，这样就可以尽快否定掉不匹配的模式
                            if p.as_char() == '~' { 
                                if let proc_macro2::TokenTree::Ident(i) = &buf[idx + 2] {
                                    if i == &self.variable_ident
                                        && prefix.span().end() == p.span().start() // 校验是否连续，无空格
                                        && p.span().end() == i.span().start()
                                    {
                                        
                                        let new_ident_litral = format!("{}{}", prefix.to_string(), n);
                                        let new_ident = proc_macro2::Ident::new(new_ident_litral.as_str(), prefix.span());
                                        ret.extend(quote::quote!(#new_ident));
                                        idx += 3; // 我们消耗了3个Token，所以这里要加3
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    // 虽然这一关要支持新的模式，可以为了通过前面的关卡，老逻辑也得兼容。
                    // 写Parser的一个通用技巧：当有多个可能冲突的规则时，优先尝试最长的
                    // 规则，因为这个规则只需要看一个Token，而上面的规则需要看3个Token，
                    // 所以这个规则要写在上一个规则的下面，否则就会导致短规则抢占，长规则无法命中。
                    if prefix == &self.variable_ident {
                        let new_ident = proc_macro2::Literal::i64_unsuffixed(n as i64);
                        ret.extend(quote::quote!(#new_ident));
                        idx += 1;
                        continue;
                    }
                    ret.extend(quote::quote!(#tree_node));
                }
                _ => {
                    // 对于其它的元素（也就是Punct和Literal），原封不动透传
                    ret.extend(quote::quote!(#tree_node));
                }
            }
            idx+=1;
        }
        ret
    }

    fn find_block_to_expand_and_do_expand(&self, c: syn::buffer::Cursor) -> (proc_macro2::TokenStream, bool) {
        let mut found = false;
        let mut ret = proc_macro2::TokenStream::new();

        let mut cursor = c;
        while !cursor.eof() {
            // 注意punct()这个函数的返回值，它会返回一个新的`Cursor`类型的值，
            // 这个新的Cursor指向了消耗掉当前标点符号以后，在TokenBuffer中的下一个位置
            // syn包提供的Cursor机制，并不是拿到一个Cursor以后，不断向后移动更新这个Cursor，
            // 而是每次都会返回给你一个全新的Cursor，新的Cursor指向新的位置，
            // 老的Cursor指向的位置保持不变
            if let Some((punct_prefix, cursor_1)) = cursor.punct(){
                if punct_prefix.as_char() == '#' {
                    if let Some((group_cur,_,cursor_2)) = cursor_1.group(proc_macro2::Delimiter::Parenthesis) {
                        if let Some((punct_suffix, cursor_3)) = cursor_2.punct() {
                            if punct_suffix.as_char() == '*' {
                                // 走到这里，说明找到了匹配的模式，按照指定的次数开始展开
                                for i in self.start..self.end {
                                    // 因为之前expand是用TokenStream这一套写的，所以
                                    // 这里还要把Cursor转换为TokenStream。毕竟是演示嘛，
                                    // 希望在最少的代码里用到最多的特性，如果是自己写的话，
                                    // 可以用Cursor的方式来写expand函数，这样这里就可以
                                    // 直接把Cursor传进去了
                                    let t = self.expand(&group_cur.token_stream(), i);
                                    ret.extend(t);
                                }
                                // 下面这行很重要，千万别忘了，把老的cursor丢了，替换成
                                // 新的，相当于把游标向前移动了
                                cursor = cursor_3;
                                found = true;
                                continue
                            }
                        }
                    }
                }
            }
            // 走到这里，说明`#(xxxxxxxxx)*`这个模式没有匹配到，那么就要按照普通代码的各个元素来处理了。

            // cursor也有用起来不方便的地方，比如在处理group的时候，我们没法统一处理()\[]\{}，需要把他们分别处理
            // 有一种暴力的做法，就是cursor提供了token_tree()方法，可以把当前游标指向的内容作为一个TokenTree返回，
            // 我们再去断言TokenTree是Group、Indet、Literal、Punct中的哪一种，这就相当于回到了上一关介绍的方法，
            // 回到了`proc_macro2`包提供的工具上去。
            // 所以我们这里本着尽量采用不重复的方式来讲解的原则，继续使用`cursor`提供的各种工具来完成本关题目
            if let Some((group_cur,_, next_cur)) = cursor.group(proc_macro2::Delimiter::Brace) {
                let (t, f) = self.find_block_to_expand_and_do_expand(group_cur);
                found = f;
                ret.extend(quote::quote!({#t}));
                cursor = next_cur;
                continue
            } else if let Some((group_cur,_, next_cur)) = cursor.group(proc_macro2::Delimiter::Bracket) {
                let (t, f) = self.find_block_to_expand_and_do_expand(group_cur);
                found = f;
                ret.extend(quote::quote!([#t]));
                cursor = next_cur;
                continue
            } else if let Some((group_cur,_, next_cur)) = cursor.group(proc_macro2::Delimiter::Parenthesis) {
                let (t, f) = self.find_block_to_expand_and_do_expand(group_cur);
                found = f;
                ret.extend(quote::quote!((#t)));
                cursor = next_cur;
                continue
            } else if let Some((punct ,next_cur)) = cursor.punct() {
                ret.extend(quote::quote!(#punct));
                cursor = next_cur;
                continue
            } else if let Some((ident ,next_cur)) = cursor.ident() {
                ret.extend(quote::quote!(#ident));
                cursor = next_cur;
                continue
            } else if let Some((literal ,next_cur)) = cursor.literal() {
                ret.extend(quote::quote!(#literal));
                cursor = next_cur;
                continue
            } else if let Some((lifetime ,next_cur)) = cursor.lifetime() {
                // lifetime这种特殊的分类也是用cursor模式来处理的时候特有的，之前`proc_macro2::TokenTree`里面没有定义这个分类
                ret.extend(quote::quote!(#lifetime));
                cursor = next_cur;
                continue
            }
        }
        (ret, found)
    }
}