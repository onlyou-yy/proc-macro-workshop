// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run

// 将 测试用例的代码复制到这里之后在根目录下运行 cargo expand 可以看到宏生成的代码
// expand 需要安转，运行 cargo install cargo-expand 安装

use derive_builder::Builder;

#[derive(Builder)]
pub struct Command {
    executable: String,
    args: Vec<String>,
    env: Vec<String>,
    current_dir: String,
}

fn main() {}
