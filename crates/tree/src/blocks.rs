//! Tree 的 block；规则见 `croon_eval::blocks`。
//! 参考：docs/Tree.md。

use croon_eval::blocks::{Block, expanded, find};

pub static BLOCKS: &[Block] = &[Block {
    name: "directory.tree",
    lang: "json",
    text: r##"{
  "type":"object",
  "description":"名字非空，不含 / 或 NUL，不是 . 或 ..",
  "additionalProperties":{"$ref":"#/$defs/entry"},
  "$defs":{
    "entry":{
      "anyOf":[
        {"type":"object","properties":{"file":{"type":"string","description":"字节 Bundle ID"},"x":{"type":"boolean"}},"required":["file","x"],"additionalProperties":false},
        {"type":"object","properties":{"dir":{"type":"string","description":"目录 Bundle ID"}},"required":["dir"],"additionalProperties":false},
        {"type":"object","properties":{"link":{"type":"string"}},"required":["link"],"additionalProperties":false}
      ]
    }
  }
}"##,
    file: "crates/tree/src/blocks.rs",
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
