//! 场景的 block：每个场景一份预期，加上场景用的假 Port；规则见 `croon_eval::blocks`。
//! 参考：docs/Kernel/Scenarios/README.md。

use croon_eval::blocks::{Block, chain, expanded, find};

pub static BLOCKS: &[Block] = croon_eval::block_files![
    "scenario.approval",
    "scenario.binding",
    "scenario.charter",
    "scenario.delegate",
    "scenario.engine",
    "scenario.exec",
    "scenario.forge",
    "scenario.pauseonly",
    "scenario.quick",
    "scenario.race",
    "scenario.reconnect",
];

/// 本 crate 的 block，找不到的去它依赖的 crate。
pub fn block(name: &str) -> Option<&'static Block> {
    chain(find(BLOCKS, name), || kernel::blocks::block(name))
}

/// 一份 block，`$block` 已展开。
pub fn json(name: &str) -> serde_json::Value {
    expanded(name, &block)
}
