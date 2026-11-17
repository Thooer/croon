//! Kernel 的 block：Instance、记录与根绑定的形状；规则见 `croon_eval::blocks`。
//! 参考：docs/Kernel/Journal.md；docs/Kernel/Instance.md；docs/Kernel/Port.md。

use croon_eval::blocks::{Block, chain, expanded, find};

pub static BLOCKS: &[Block] = croon_eval::block_files![
    "instance.delivery",
    "instance.id",
    "instance.record",
    "reference.binding",
    "reference.look.result",
];

/// 本 crate 的 block，找不到的去它依赖的 crate。
pub fn block(name: &str) -> Option<&'static Block> {
    chain(find(BLOCKS, name), || croon_port::blocks::block(name))
}

/// 一份 block，`$block` 已展开。
pub fn json(name: &str) -> serde_json::Value {
    expanded(name, &block)
}
