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

/// 在几张 block 表里按名字取一份 block，并展开其中的 `$block`；表按先后找。
///
/// 给把 block 留着 `$block` 写进 schema 文件、加载时再展开的使用方。规则与文档里的 block 相同：
/// `{"$block": "名字"}` 或 `{"$block": "名字#/JSON Pointer"}` 换成那份 block 或其中的片段，
/// 与 `$block` 并列的键覆盖展开出的同名键；宿主自己的 `$ref` 不变。展开出的片段里，`$ref`
/// 所属那份 Schema 的根严格在片段内部的，连同那个根上的 `$defs` 原样保留；其余的按它在原 block
/// 里的意思内联，片段顶层的 `$defs` 删掉。`$ref` 所属的根，是离它最近、能解析这个指针的祖先。
///
/// 名字找不到、指针不通、`$block` 成环时 panic：block 是编进来的数据，错了就是构建错了。
pub fn expanded(tables: &[&'static [Block]], name: &str) -> Json {
    let raw = parse(find(tables, name));
    expand(tables, &raw, &mut vec![name.to_owned()])
}

fn find(tables: &[&'static [Block]], name: &str) -> &'static Block {
    tables
        .iter()
        .flat_map(|table| table.iter())
        .find(|block| block.name == name)
        .unwrap_or_else(|| panic!("没有名为 {name} 的 block"))
}

fn parse(block: &Block) -> Json {
    serde_json::from_str(block.text).unwrap_or_else(|e| {
        panic!(
            "{}:{}：block `{}` 不是合法的 JSON：{e}",
            block.file, block.line, block.name
        )
    })
}

fn segments(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .skip(1)
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn at<'a>(value: &'a Json, path: &[String]) -> Option<&'a Json> {
    path.iter().try_fold(value, |node, key| match node {
        Json::Object(map) => map.get(key),
        Json::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
        _ => None,
    })
}

/// 展开 `value` 里的每个 `$block`；`stack` 是正在展开的「名字#指针」，用来认出环。
fn expand(tables: &[&'static [Block]], value: &Json, stack: &mut Vec<String>) -> Json {
    match value {
        Json::Object(map) => {
            let Some(target) = map.get("$block") else {
                return Json::Object(
                    map.iter()
                        .map(|(k, v)| (k.clone(), expand(tables, v, stack)))
                        .collect(),
                );
            };
            let target = target
                .as_str()
                .unwrap_or_else(|| panic!("$block 要写字符串：{target}"));
            if stack.iter().any(|s| s == target) {
                panic!("$block 成环：{} → {target}", stack.join(" → "));
            }
            let (name, pointer) = target.split_once('#').unwrap_or((target, ""));
            let whole = parse(find(tables, name));
            let path = segments(pointer);
            let node = at(&whole, &path).unwrap_or_else(|| panic!("{target} 指向的位置不存在"));
            let mut fragment = localize(&whole, &path, node, &path, 0);
            if let Json::Object(top) = &mut fragment {
                top.remove("$defs");
            }
            stack.push(target.to_owned());
            let mut out = expand(tables, &fragment, stack);
            stack.pop();
            let Json::Object(out_map) = &mut out else {
                if map.len() > 1 {
                    panic!("{target} 展开出的不是对象，不能再覆盖键");
                }
                return out;
            };
            for (k, v) in map {
                if k != "$block" {
                    out_map.insert(k.clone(), expand(tables, v, stack));
                }
            }
            out
        }
        Json::Array(items) => Json::Array(items.iter().map(|v| expand(tables, v, stack)).collect()),
        other => other.clone(),
    }
}

/// 片段 `fragment`（在 `whole` 里的位置）中位于 `here` 的 `node`：所属根不严格在片段内部的 `$ref` 内联。
fn localize(whole: &Json, fragment: &[String], node: &Json, here: &[String], depth: usize) -> Json {
    if depth > 64 {
        panic!("$ref 内联超过 64 层，多半是递归的 Schema");
    }
    match node {
        Json::Object(map) => {
            let reference = map
                .get("$ref")
                .and_then(Json::as_str)
                .and_then(|r| r.strip_prefix('#'));
            if let Some(pointer) = reference {
                let target = segments(pointer);
                let root = (0..=here.len())
                    .rev()
                    .map(|n| &here[..n])
                    .find(|root| at(whole, root).and_then(|r| at(r, &target)).is_some())
                    .unwrap_or_else(|| panic!("$ref #{pointer} 解析不到"));
                let inside = root.len() > fragment.len() && root.starts_with(fragment);
                if !inside {
                    let absolute: Vec<String> = root.iter().chain(&target).cloned().collect();
                    let mut out = localize(
                        whole,
                        fragment,
                        at(whole, &absolute).unwrap(),
                        &absolute,
                        depth + 1,
                    );
                    if let Json::Object(out_map) = &mut out {
                        for (k, v) in map {
                            if k != "$ref" {
                                out_map
                                    .insert(k.clone(), child(whole, fragment, v, here, k, depth));
                            }
                        }
                    }
                    return out;
                }
            }
            Json::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), child(whole, fragment, v, here, k, depth)))
                    .collect(),
            )
        }
        Json::Array(items) => Json::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| child(whole, fragment, v, here, &i.to_string(), depth))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn child(
    whole: &Json,
    fragment: &[String],
    value: &Json,
    here: &[String],
    key: &str,
    depth: usize,
) -> Json {
    let mut path = here.to_vec();
    path.push(key.to_owned());
    localize(whole, fragment, value, &path, depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    static PORT: &[Block] = &[Block {
        name: "port",
        lang: "json",
        text: r##"{"in":{"type":"object","properties":{"m":{"$ref":"#/$defs/m"}},"$defs":{"m":{"type":"string"}}}}"##,
        file: "test",
        line: 0,
    }];

    #[test]
    fn a_whole_block_keeps_refs_whose_root_is_inside_and_a_fragment_inlines_them() {
        let whole = expand(
            &[PORT],
            &json!({"p": {"$block": "port"}, "$ref": "#/x"}),
            &mut vec![],
        );
        assert_eq!(
            whole["p"],
            serde_json::from_str::<Json>(PORT[0].text).unwrap()
        );
        assert_eq!(whole["$ref"], "#/x", "宿主自己的 $ref 不变");
        let part = expand(
            &[PORT],
            &json!({"$block": "port#/in", "description": "d"}),
            &mut vec![],
        );
        assert_eq!(
            part,
            json!({"type":"object","properties":{"m":{"type":"string"}},"description":"d"})
        );
    }

    #[test]
    fn a_block_may_point_into_itself_but_not_in_a_cycle() {
        static SELF: &[Block] = &[Block {
            name: "self",
            lang: "json",
            text: r#"{"a":{"x":1},"b":{"$block":"self#/a"}}"#,
            file: "test",
            line: 0,
        }];
        assert_eq!(expanded(&[SELF], "self")["b"], json!({"x": 1}));
        static LOOP: &[Block] = &[Block {
            name: "loop",
            lang: "json",
            text: r#"{"a":{"$block":"loop#/a"}}"#,
            file: "test",
            line: 0,
        }];
        assert!(std::panic::catch_unwind(|| expanded(&[LOOP], "loop")).is_err());
    }
}
