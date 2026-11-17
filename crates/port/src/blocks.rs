//! Port 的 block：Kernel 的 Port 表；规则见 `croon_eval::blocks`。
//! 参考：docs/Kernel/Port.md。

use croon_eval::blocks::{Block, expanded, find};

pub static BLOCKS: &[Block] = &[Block {
    name: "reference.kernel",
    lang: "json",
    text: include_str!("../schema/reference.kernel.json"),
    file: "crates/port/schema/reference.kernel.json",
    line: 0,
}];

/// 本 crate 的 block，找不到的去它依赖的 crate。
pub fn block(name: &str) -> Option<&'static Block> {
    find(BLOCKS, name).or_else(|| croon_eval::blocks::block(name))
}

/// 一份 block，`$block` 已展开。
pub fn json(name: &str) -> serde_json::Value {
    expanded(name, &block)
}
