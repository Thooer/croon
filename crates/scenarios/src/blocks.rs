//! block 表：schema/ 下的 JSON 文件直接编进来，不经过 build.rs。
//! 参考：docs/Kernel/Engineering.md（接口）。

pub use croon_eval::blocks::{Block, get_in, json_in};

pub static BLOCKS: &[Block] = &[
    Block {
        name: "definition.eval",
        lang: "json",
        text: include_str!("../schema/definition.eval.json"),
        file: "crates/scenarios/schema/definition.eval.json",
        line: 0,
    },
    Block {
        name: "definition.failure",
        lang: "json",
        text: include_str!("../schema/definition.failure.json"),
        file: "crates/scenarios/schema/definition.failure.json",
        line: 0,
    },
    Block {
        name: "definition.outcome",
        lang: "json",
        text: croon_port::blocks::DEFINITION_OUTCOME,
        file: "crates/port/schema/definition.outcome.json",
        line: 0,
    },
    Block {
        name: "definition.schema",
        lang: "json",
        text: include_str!("../schema/definition.schema.json"),
        file: "crates/scenarios/schema/definition.schema.json",
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
        name: "instance.id",
        lang: "json",
        text: include_str!("../schema/instance.id.json"),
        file: "crates/scenarios/schema/instance.id.json",
        line: 0,
    },
    Block {
        name: "instance.record",
        lang: "json",
        text: include_str!("../schema/instance.record.json"),
        file: "crates/scenarios/schema/instance.record.json",
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
        name: "reference.binding",
        lang: "json",
        text: include_str!("../schema/reference.binding.json"),
        file: "crates/scenarios/schema/reference.binding.json",
        line: 0,
    },
    Block {
        name: "reference.kernel",
        lang: "json",
        text: croon_port::blocks::REFERENCE_KERNEL,
        file: "crates/port/schema/reference.kernel.json",
        line: 0,
    },
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
        name: "scenario.delegate",
        lang: "json",
        text: include_str!("../schema/scenario.delegate.json"),
        file: "crates/scenarios/schema/scenario.delegate.json",
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
    Block {
        name: "value.bundle",
        lang: "json",
        text: include_str!("../schema/value.bundle.json"),
        file: "crates/scenarios/schema/value.bundle.json",
        line: 0,
    },
    Block {
        name: "value.delivery",
        lang: "json",
        text: include_str!("../schema/value.delivery.json"),
        file: "crates/scenarios/schema/value.delivery.json",
        line: 0,
    },
    Block {
        name: "value.limits",
        lang: "json",
        text: include_str!("../schema/value.limits.json"),
        file: "crates/scenarios/schema/value.limits.json",
        line: 0,
    },
    Block {
        name: "value.schema",
        lang: "json",
        text: include_str!("../schema/value.schema.json"),
        file: "crates/scenarios/schema/value.schema.json",
        line: 0,
    },
];

pub fn get(name: &str) -> &'static Block {
    get_in(BLOCKS, name)
}

pub fn json(name: &str) -> serde_json::Value {
    json_in(BLOCKS, name)
}
