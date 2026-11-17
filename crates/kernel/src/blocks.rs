//! Kernel 的 block：Instance、记录与根绑定的形状；规则见 `croon_eval::blocks`。
//! 参考：docs/Kernel/Journal.md；docs/Kernel/Instance.md；docs/Kernel/Port.md。

use croon_eval::blocks::{Block, expanded, find};

pub static BLOCKS: &[Block] = &[
    Block {
        name: "instance.delivery",
        lang: "json",
        text: r#"{
  "type":"object",
  "properties":{"value":{"type":"string","description":"Value ID"},"dependencies":{"$block":"value.dependencies"}},
  "required":["value","dependencies"],"additionalProperties":false
}"#,
        file: "crates/kernel/src/blocks.rs",
        line: 0,
    },
    Block {
        name: "instance.id",
        lang: "json",
        text: r#"{
  "prefix": { "instance": "i", "handle": "h", "event": "e" },
  "separator": "-",
  "alphabet": "0123456789abcdefghijklmnopqrstuvwxyz",
  "max_length": 80
}"#,
        file: "crates/kernel/src/blocks.rs",
        line: 0,
    },
    Block {
        name: "instance.record",
        lang: "json",
        text: include_str!("../schema/instance.record.json"),
        file: "crates/kernel/schema/instance.record.json",
        line: 0,
    },
    Block {
        name: "reference.binding",
        lang: "json",
        text: r#"{
  "type":"object",
  "properties":{
    "sources":{
      "type":"object",
      "additionalProperties":{
        "anyOf":[
          {"type":"object","properties":{"kernel":{"enum":["pack","construct","instantiate"]},"spec":{"type":"object"}},"required":["kernel","spec"],"additionalProperties":false},
          {"type":"object","properties":{"host":{"type":"string","description":"宿主恢复身份"},"spec":{"type":"object"}},"required":["host","spec"],"additionalProperties":false}
        ]
      }
    }
  },
  "required":["sources"],"additionalProperties":false
}"#,
        file: "crates/kernel/src/blocks.rs",
        line: 0,
    },
    Block {
        name: "reference.look.result",
        lang: "json",
        text: r#"{
  "anyOf":[
    {"type":"object","properties":{"status":{"enum":["running","paused","ended"]},"outcome":{"anyOf":[{"$block":"definition.outcome"},{"type":"null"}]}},"required":["status","outcome"],"additionalProperties":false},
    {"type":"object","properties":{"records":{"type":"array","items":{"$block":"instance.record"}}},"required":["records"],"additionalProperties":false}
  ]
}"#,
        file: "crates/kernel/src/blocks.rs",
        line: 0,
    },
];

/// 本 crate 的 block，找不到的去它依赖的 crate。
pub fn block(name: &str) -> Option<&'static Block> {
    find(BLOCKS, name).or_else(|| croon_port::blocks::block(name))
}

/// 一份 block，`$block` 已展开。
pub fn json(name: &str) -> serde_json::Value {
    expanded(name, &block)
}
