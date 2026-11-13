//! block 表：schema/ 下的 JSON 文件直接编进来，不经过 build.rs。
//! 参考：docs/Kernel/Engineering.md（接口）。

pub use croon_eval::blocks::{Block, get_in, json_in};

pub static BLOCKS: &[Block] = &[
    Block { name: "definition.eval", lang: "json", text: include_str!("../schema/definition.eval.json"), file: "crates/kernel/schema/definition.eval.json", line: 0 },
    Block { name: "definition.failure", lang: "json", text: include_str!("../schema/definition.failure.json"), file: "crates/kernel/schema/definition.failure.json", line: 0 },
    Block { name: "definition.outcome", lang: "json", text: include_str!("../schema/definition.outcome.json"), file: "crates/kernel/schema/definition.outcome.json", line: 0 },
    Block { name: "instance.id", lang: "json", text: include_str!("../schema/instance.id.json"), file: "crates/kernel/schema/instance.id.json", line: 0 },
    Block { name: "instance.record", lang: "json", text: include_str!("../schema/instance.record.json"), file: "crates/kernel/schema/instance.record.json", line: 0 },
    Block { name: "reference.binding", lang: "json", text: include_str!("../schema/reference.binding.json"), file: "crates/kernel/schema/reference.binding.json", line: 0 },
    Block { name: "reference.kernel", lang: "json", text: include_str!("../schema/reference.kernel.json"), file: "crates/kernel/schema/reference.kernel.json", line: 0 },
    Block { name: "value.bundle", lang: "json", text: include_str!("../schema/value.bundle.json"), file: "crates/kernel/schema/value.bundle.json", line: 0 },
    Block { name: "value.delivery", lang: "json", text: include_str!("../schema/value.delivery.json"), file: "crates/kernel/schema/value.delivery.json", line: 0 },
];

pub fn get(name: &str) -> &'static Block {
    get_in(BLOCKS, name)
}

pub fn json(name: &str) -> serde_json::Value {
    json_in(BLOCKS, name)
}
