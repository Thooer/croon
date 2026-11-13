//! block 表：schema/ 下的 JSON 文件直接编进来。
//! 参考：docs/Kernel/Port.md；docs/Kernel/Definition/README.md。

pub use croon_eval::blocks::{Block, get_in, json_in};

pub const DEFINITION_OUTCOME: &str = include_str!("../schema/definition.outcome.json");
pub const REFERENCE_KERNEL: &str = include_str!("../schema/reference.kernel.json");

pub static BLOCKS: &[Block] = &[
    Block {
        name: "definition.outcome",
        lang: "json",
        text: DEFINITION_OUTCOME,
        file: "crates/port/schema/definition.outcome.json",
        line: 0,
    },
    Block {
        name: "reference.kernel",
        lang: "json",
        text: REFERENCE_KERNEL,
        file: "crates/port/schema/reference.kernel.json",
        line: 0,
    },
];

pub fn get(name: &str) -> &'static Block {
    get_in(BLOCKS, name)
}
pub fn json(name: &str) -> serde_json::Value {
    json_in(BLOCKS, name)
}
