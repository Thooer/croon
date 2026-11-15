//! block 表：schema/ 下的 JSON 文件直接编进来，不经过 build.rs。
//! 参考：docs/Kernel/Engineering.md（接口）。

use serde_json::Value as Json;

/// 一个带名字的 block；`file` 与 `line` 报错时指回来源。
#[derive(Debug)]
pub struct Block {
    pub name: &'static str,
    pub lang: &'static str,
    pub text: &'static str,
    pub file: &'static str,
    pub line: usize,
}

pub static BLOCKS: &[Block] = &[
    Block {
        name: "definition.schema",
        lang: "json",
        text: include_str!("../schema/definition.schema.json"),
        file: "crates/eval/schema/definition.schema.json",
        line: 0,
    },
    Block {
        name: "value.bundle",
        lang: "json",
        text: include_str!("../schema/value.bundle.json"),
        file: "crates/eval/schema/value.bundle.json",
        line: 0,
    },
    Block {
        name: "value.limits",
        lang: "json",
        text: include_str!("../schema/value.limits.json"),
        file: "crates/eval/schema/value.limits.json",
        line: 0,
    },
    Block {
        name: "value.schema",
        lang: "json",
        text: include_str!("../schema/value.schema.json"),
        file: "crates/eval/schema/value.schema.json",
        line: 0,
    },
];

pub fn get(name: &str) -> &'static Block {
    get_in(BLOCKS, name)
}

pub fn get_in(blocks: &'static [Block], name: &str) -> &'static Block {
    blocks
        .iter()
        .find(|block| block.name == name)
        .unwrap_or_else(|| panic!("文档里没有名为 {name} 的 block"))
}

pub fn json(name: &str) -> Json {
    json_in(BLOCKS, name)
}

pub fn json_in(blocks: &'static [Block], name: &str) -> Json {
    let block = get_in(blocks, name);
    serde_json::from_str(block.text).unwrap_or_else(|e| {
        panic!(
            "{}:{}：block `{}` 不是合法的 JSON：{e}",
            block.file, block.line, block.name
        )
    })
}
