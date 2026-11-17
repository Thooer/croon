//! 场景的 block：每个场景一份预期，加上场景用的假 Port；规则见 `croon_eval::blocks`。
//! 参考：docs/Kernel/Scenarios/README.md。

use croon_eval::blocks::{Block, expanded, find};

pub static BLOCKS: &[Block] = &[
    Block {
        name: "scenario.approval",
        lang: "json",
        text: include_str!("../schema/scenario.approval.json"),
        file: "crates/scenarios/schema/scenario.approval.json",
        line: 0,
    },
    Block {
        name: "scenario.binding",
        lang: "json",
        text: include_str!("../schema/scenario.binding.json"),
        file: "crates/scenarios/schema/scenario.binding.json",
        line: 0,
    },
    Block {
        name: "scenario.charter",
        lang: "json",
        text: include_str!("../schema/scenario.charter.json"),
        file: "crates/scenarios/schema/scenario.charter.json",
        line: 0,
    },
    Block {
        name: "scenario.delegate",
        lang: "json",
        text: include_str!("../schema/scenario.delegate.json"),
        file: "crates/scenarios/schema/scenario.delegate.json",
        line: 0,
    },
    Block {
        name: "scenario.engine",
        lang: "json",
        text: include_str!("../schema/scenario.engine.json"),
        file: "crates/scenarios/schema/scenario.engine.json",
        line: 0,
    },
    Block {
        name: "scenario.exec",
        lang: "json",
        text: include_str!("../schema/scenario.exec.json"),
        file: "crates/scenarios/schema/scenario.exec.json",
        line: 0,
    },
    Block {
        name: "scenario.forge",
        lang: "json",
        text: include_str!("../schema/scenario.forge.json"),
        file: "crates/scenarios/schema/scenario.forge.json",
        line: 0,
    },
    Block {
        name: "scenario.pauseonly",
        lang: "json",
        text: include_str!("../schema/scenario.pauseonly.json"),
        file: "crates/scenarios/schema/scenario.pauseonly.json",
        line: 0,
    },
    Block {
        name: "scenario.quick",
        lang: "json",
        text: include_str!("../schema/scenario.quick.json"),
        file: "crates/scenarios/schema/scenario.quick.json",
        line: 0,
    },
    Block {
        name: "scenario.race",
        lang: "json",
        text: include_str!("../schema/scenario.race.json"),
        file: "crates/scenarios/schema/scenario.race.json",
        line: 0,
    },
    Block {
        name: "scenario.reconnect",
        lang: "json",
        text: include_str!("../schema/scenario.reconnect.json"),
        file: "crates/scenarios/schema/scenario.reconnect.json",
        line: 0,
    },
];

/// 本 crate 的 block，找不到的去它依赖的 crate。
pub fn block(name: &str) -> Option<&'static Block> {
    find(BLOCKS, name).or_else(|| kernel::blocks::block(name))
}

/// 一份 block，`$block` 已展开。
pub fn json(name: &str) -> serde_json::Value {
    expanded(name, &block)
}
