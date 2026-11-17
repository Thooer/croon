//! block：代码取用的 JSON 形状，各有一个名字，是那份形状的唯一出处；文档按名字引用。
//! 每份在用它的 crate 的 schema/ 下各成一个文件，文件名就是 block 名；文件里留着 `$block`，
//! 取用时由 [`expanded`] 展开；各 crate 的 `blocks::block` 先找自己的表，再找它依赖的 crate。
//! 本 crate 的表收 Value、Schema、Definition 与求值会话的形状。
//! 参考：docs/Kernel/Engineering.md「选择」。

use serde_json::Value as Json;

/// 一个带名字的 block：`text` 是 `schema/<名字>.json` 的内容，`file` 在报错时指回它。
#[derive(Debug)]
pub struct Block {
    pub name: &'static str,
    pub text: &'static str,
    pub file: &'static str,
}

/// 列出 block 的名字，每个名字对应调用它的 crate 里的 `schema/<名字>.json`，编进来成一张表。
/// 名字就是去哪找它：先看用它的 crate 的 `schema/`，再沿依赖往下找。
#[macro_export]
macro_rules! block_files {
    ($($name:literal),* $(,)?) => {
        &[$($crate::blocks::Block {
            name: $name,
            text: include_str!(concat!("../schema/", $name, ".json")),
            file: concat!("schema/", $name, ".json"),
        }),*]
    };
}

pub static BLOCKS: &[Block] = block_files![
    "definition.eval",
    "definition.failure",
    "definition.outcome",
    "definition.schema",
    "value.bundle",
    "value.delivery",
    "value.dependencies",
    "value.limits",
    "value.schema",
];

/// 在一张表里按名字找 block。
pub fn find(table: &'static [Block], name: &str) -> Option<&'static Block> {
    table.iter().find(|block| block.name == name)
}

/// 自己的表与上游的结果合起来：两边是两份不同的 block 用了同一个名字是构建错误，不让下游悄悄盖掉上游。
/// 同一份 block 经几条依赖路径各找到一次不算重名。
pub fn chain(
    own: Option<&'static Block>,
    upstream: impl FnOnce() -> Option<&'static Block>,
) -> Option<&'static Block> {
    match (own, upstream()) {
        (Some(a), Some(b)) if std::ptr::eq(a, b) => Some(a),
        (Some(a), Some(_)) => panic!("block {} 与上游重名", a.name),
        (Some(block), None) | (None, Some(block)) => Some(block),
        (None, None) => None,
    }
}

/// 本 crate 的 block。
pub fn block(name: &str) -> Option<&'static Block> {
    find(BLOCKS, name)
}

/// 本 crate 的一份 block，`$block` 已展开。
pub fn json(name: &str) -> Json {
    expanded(name, &block)
}

/// 按名字取一份 block，展开其中的 `$block`；`lookup` 是取用方够得到的 block：
/// 惯例是各 crate 的 `blocks::block`，先找自己的表，再找它依赖的 crate。
///
/// 展开规则以这里为准：`{"$block": "名字"}` 或 `{"$block": "名字#/JSON Pointer"}` 换成那份 block
/// 或其中的片段，与 `$block` 并列的键覆盖展开出的同名键；宿主自己的 `$ref` 不变。展开出的片段里，
/// `$ref` 所属那份 Schema 的根严格在片段内部的，连同那个根上的 `$defs` 原样保留；其余的按它在原
/// block 里的意思内联，片段顶层的 `$defs` 删掉。`$ref` 所属的根，是离它最近、能解析这个指针的祖先。
///
/// 名字找不到、指针不通、`$block` 成环时 panic：block 是编进来的数据，错了就是构建错了。
pub fn expanded(name: &str, lookup: &dyn Fn(&str) -> Option<&'static Block>) -> Json {
    let raw = parse(get(lookup, name));
    expand(lookup, &raw, &mut vec![name.to_owned()])
}

fn get(lookup: &dyn Fn(&str) -> Option<&'static Block>, name: &str) -> &'static Block {
    lookup(name).unwrap_or_else(|| panic!("没有名为 {name} 的 block"))
}

fn parse(block: &Block) -> Json {
    serde_json::from_str(block.text).unwrap_or_else(|e| {
        panic!(
            "{}：block `{}` 不是合法的 JSON：{e}",
            block.file, block.name
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
fn expand(
    lookup: &dyn Fn(&str) -> Option<&'static Block>,
    value: &Json,
    stack: &mut Vec<String>,
) -> Json {
    match value {
        Json::Object(map) => {
            let Some(target) = map.get("$block") else {
                return Json::Object(
                    map.iter()
                        .map(|(k, v)| (k.clone(), expand(lookup, v, stack)))
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
            let whole = parse(get(lookup, name));
            let path = segments(pointer);
            let node = at(&whole, &path).unwrap_or_else(|| panic!("{target} 指向的位置不存在"));
            let mut fragment = localize(&whole, &path, node, &path, 0);
            if let Json::Object(top) = &mut fragment {
                top.remove("$defs");
            }
            stack.push(target.to_owned());
            let mut out = expand(lookup, &fragment, stack);
            stack.pop();
            let Json::Object(out_map) = &mut out else {
                if map.len() > 1 {
                    panic!("{target} 展开出的不是对象，不能再覆盖键");
                }
                return out;
            };
            for (k, v) in map {
                if k != "$block" {
                    out_map.insert(k.clone(), expand(lookup, v, stack));
                }
            }
            out
        }
        Json::Array(items) => Json::Array(items.iter().map(|v| expand(lookup, v, stack)).collect()),
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
        text: r##"{"in":{"type":"object","properties":{"m":{"$ref":"#/$defs/m"}},"$defs":{"m":{"type":"string"}}}}"##,
        file: "test",
    }];

    #[test]
    fn a_whole_block_keeps_refs_whose_root_is_inside_and_a_fragment_inlines_them() {
        let whole = expand(
            &|n| find(PORT, n),
            &json!({"p": {"$block": "port"}, "$ref": "#/x"}),
            &mut vec![],
        );
        assert_eq!(
            whole["p"],
            serde_json::from_str::<Json>(PORT[0].text).unwrap()
        );
        assert_eq!(whole["$ref"], "#/x", "宿主自己的 $ref 不变");
        let part = expand(
            &|n| find(PORT, n),
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
            text: r#"{"a":{"x":1},"b":{"$block":"self#/a"}}"#,
            file: "test",
        }];
        assert_eq!(expanded("self", &|n| find(SELF, n))["b"], json!({"x": 1}));
        static LOOP: &[Block] = &[Block {
            name: "loop",
            text: r#"{"a":{"$block":"loop#/a"}}"#,
            file: "test",
        }];
        assert!(std::panic::catch_unwind(|| expanded("loop", &|n| find(LOOP, n))).is_err());
    }
}
