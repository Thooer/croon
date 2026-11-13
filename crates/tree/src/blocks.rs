//! block 表：schema/ 下的 JSON 文件直接编进来。
//! 参考：docs/Tree.md。

use croon_eval::blocks::{Block, json_in};

pub static BLOCKS: &[Block] = &[Block {
    name: "directory.tree",
    lang: "json",
    text: include_str!("../schema/directory.tree.json"),
    file: "crates/tree/schema/directory.tree.json",
    line: 0,
}];

pub fn json(name: &str) -> serde_json::Value {
    json_in(BLOCKS, name)
}
