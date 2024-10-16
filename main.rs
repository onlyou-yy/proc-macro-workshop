// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run

// 将 测试用例的代码复制到这里之后在根目录下运行 cargo expand 可以看到宏生成的代码
// expand 需要安转，运行 cargo install cargo-expand 安装

use derive_debug::CustomDebug;
use std::fmt::Debug;

pub trait Trait {
    type Value;
}

#[derive(CustomDebug)]
#[debug(bound = "T::Value: Debug")]
pub struct Wrapper<T: Trait> {
    field: Field<T>,
}

#[derive(CustomDebug)]
struct Field<T: Trait> {
    values: Vec<T::Value>,
}

fn assert_debug<F: Debug>() {}

fn main() {
    struct Id;

    impl Trait for Id {
        type Value = u8;
    }

    assert_debug::<Wrapper<Id>>();
}

