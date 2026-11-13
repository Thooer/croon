//! 展开 JSON block 里的 `$block`。
//! 参考：docs/Kernel/Engineering.md（接口）。
//!
//! 宿主的 `$ref` 与 `$defs` 原样保留。展开出的内容里，`$ref` 所属那份 Schema 的根严格在片段内部的，
//! 连同那个根上的 `$defs` 原样保留；其余的按所属 Schema 顶层的 `$defs` 解开，`$defs` 随之删掉。
//! 所以展开出的内容放进宿主仍自身完整，引用一份自身完整的片段得到的就是原文。

use std::collections::BTreeMap;

use serde_json::{Map, Value as Json};

use crate::Block;

/// 一处展开不了：`at` 是 `$block` 或 `$ref` 写在 `name` 这个 block 里的 JSON Pointer。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{file}:{line}: block `{name}` 的 {at}：{message}")]
pub struct ExpandError {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub at: String,
    pub message: String,
}

/// 展开每个 JSON block 里的 `$block`。没写 `$block` 的 block 原文不变；写了的换成展开后的文本。
/// 不是 JSON 或解析不了的 block 原样留着，被 `$block` 指到时才报错。
pub fn expand(
    blocks: &BTreeMap<String, Block>,
) -> Result<BTreeMap<String, Block>, Vec<ExpandError>> {
    let parsed: BTreeMap<&str, Json> = blocks
        .values()
        .filter(|b| b.lang == "json")
        .filter_map(|b| Some((b.name.as_str(), serde_json::from_str(&b.text).ok()?)))
        .collect();
    let cx = Cx {
        blocks,
        parsed: &parsed,
    };

    let mut out = blocks.clone();
    let mut errors: Vec<ExpandError> = Vec::new();
    for (name, root) in &parsed {
        let mut stack = vec![key(name, &[])];
        match cx.walk(name, &[], root, None, &mut stack) {
            Ok(expanded) if expanded != *root => {
                let text = serde_json::to_string_pretty(&expanded).expect("JSON 总能写成文本");
                out.get_mut(*name).expect("取自同一张表").text = text + "\n";
            }
            Ok(_) => {}
            Err(error) if !errors.contains(&error) => errors.push(error),
            Err(_) => {}
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

struct Cx<'a> {
    blocks: &'a BTreeMap<String, Block>,
    parsed: &'a BTreeMap<&'a str, Json>,
}

impl Cx<'_> {
    /// 展开 `block` 里位于 `path` 的 `node`。`fragment` 是 `Some` 时 `node` 是展开出的内容，
    /// 来自 `block` 里这个位置的片段：所属 Schema 的根不严格在片段内部的 `$ref` 解开，`$defs` 删掉。
    /// `stack` 是正在展开的位置，再遇到就是成环。
    fn walk(
        &self,
        block: &str,
        path: &[String],
        node: &Json,
        fragment: Option<&[String]>,
        stack: &mut Vec<String>,
    ) -> Result<Json, ExpandError> {
        let map = match node {
            Json::Object(map) => map,
            Json::Array(items) => {
                return items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        self.walk(block, &child(path, &i.to_string()), item, fragment, stack)
                    })
                    .collect::<Result<_, _>>()
                    .map(Json::Array);
            }
            _ => return Ok(node.clone()),
        };
        let fail = |message: String| self.error(block, path, message);

        if let Some(reference) = map.get("$block") {
            if map.contains_key("$ref") {
                return Err(fail("$block 与 $ref 不能并列".to_owned()));
            }
            let reference = reference.as_str().ok_or_else(|| {
                fail("$block 的值要是字符串：名字，或 名字#JSON Pointer".to_owned())
            })?;
            let (name, pointer) = reference.split_once('#').unwrap_or((reference, ""));
            let target_path = segments(pointer)
                .ok_or_else(|| fail(format!("$block {reference} 的 Pointer 要以 / 开头")))?;
            let root = self.parsed.get(name).ok_or_else(|| {
                fail(format!("$block {reference}：没有名为 {name} 的 JSON block"))
            })?;
            let target = descend(root, &target_path).ok_or_else(|| {
                fail(format!(
                    "$block {reference}：Pointer 在 {name} 里解析不到（只经过对象的键）"
                ))
            })?;
            let expanded = self.enter(name, &target_path, &target_path, target, stack, || {
                fail(format!("$block {reference} 成环"))
            })?;
            return self.overlay(block, path, map, "$block", expanded, fragment, stack);
        }

        if let Some(fragment) = fragment
            && let Some(reference) = map.get("$ref")
        {
            let def = reference
                .as_str()
                .and_then(|r| r.strip_prefix("#/$defs/"))
                .ok_or_else(|| fail(format!("$ref {reference} 要写作 #/$defs/名字")))?;
            let root = &self.parsed[block];
            let schema = schema_root(root, path)
                .ok_or_else(|| fail(format!("$ref {reference} 找不到所属 Schema 顶层的 $defs")))?;
            if strictly_inside(schema, fragment) {
                return self.copy(block, path, map, Some(fragment), stack);
            }
            let def_path = [schema, &["$defs".to_owned(), def.to_owned()]].concat();
            let target = descend(root, &def_path).ok_or_else(|| {
                fail(format!(
                    "$ref {reference} 在所属 Schema 顶层的 $defs 里找不到"
                ))
            })?;
            let expanded = self.enter(block, &def_path, fragment, target, stack, || {
                fail(format!("$ref {reference} 成环：递归的形状展开不了"))
            })?;
            return self.overlay(block, path, map, "$ref", expanded, Some(fragment), stack);
        }

        self.copy(block, path, map, fragment, stack)
    }

    /// 逐个键展开一个对象；要删的 `$defs` 不写出。
    fn copy(
        &self,
        block: &str,
        path: &[String],
        map: &Map<String, Json>,
        fragment: Option<&[String]>,
        stack: &mut Vec<String>,
    ) -> Result<Json, ExpandError> {
        let mut out = Map::new();
        for (k, v) in map {
            if k == "$defs" && drops_defs(fragment, path) {
                continue;
            }
            out.insert(
                k.clone(),
                self.walk(block, &child(path, k), v, fragment, stack)?,
            );
        }
        Ok(Json::Object(out))
    }

    /// 展开 `block` 里 `path` 处的内容，它属于 `fragment` 这个片段；它已在 `stack` 上就是成环。
    fn enter(
        &self,
        block: &str,
        path: &[String],
        fragment: &[String],
        target: &Json,
        stack: &mut Vec<String>,
        cycle: impl FnOnce() -> ExpandError,
    ) -> Result<Json, ExpandError> {
        let here = key(block, path);
        if stack.contains(&here) {
            return Err(cycle());
        }
        stack.push(here);
        let expanded = self.walk(block, path, target, Some(fragment), stack);
        stack.pop();
        expanded
    }

    /// 与 `keyword` 并列的键覆盖展开出的同名键。
    #[allow(clippy::too_many_arguments)]
    fn overlay(
        &self,
        block: &str,
        path: &[String],
        map: &Map<String, Json>,
        keyword: &str,
        expanded: Json,
        fragment: Option<&[String]>,
        stack: &mut Vec<String>,
    ) -> Result<Json, ExpandError> {
        let siblings: Vec<_> = map
            .iter()
            .filter(|(k, _)| *k != keyword && !(*k == "$defs" && drops_defs(fragment, path)))
            .collect();
        if siblings.is_empty() {
            return Ok(expanded);
        }
        let Json::Object(mut base) = expanded else {
            return Err(self.error(
                block,
                path,
                format!("{keyword} 有并列的键，展开出的却不是对象，没有可覆盖的键"),
            ));
        };
        for (k, v) in siblings {
            base.insert(
                k.clone(),
                self.walk(block, &child(path, k), v, fragment, stack)?,
            );
        }
        Ok(Json::Object(base))
    }

    fn error(&self, block: &str, path: &[String], message: String) -> ExpandError {
        let source = &self.blocks[block];
        ExpandError {
            name: block.to_owned(),
            file: source.file.clone(),
            line: source.line,
            at: pointer(path),
            message,
        }
    }
}

/// 离 `path` 最近的、顶层写着 `$defs` 的那份 Schema 的位置。`properties` 与 `$defs` 本身是名字的表，不算。
fn schema_root<'p>(root: &Json, path: &'p [String]) -> Option<&'p [String]> {
    (0..=path.len()).rev().map(|i| &path[..i]).find(|prefix| {
        !matches!(
            prefix.last().map(String::as_str),
            Some("properties" | "$defs")
        ) && descend(root, prefix)
            .is_some_and(|node| node.get("$defs").is_some_and(Json::is_object))
    })
}

/// `inner` 是否严格在 `outer` 内部。
fn strictly_inside(inner: &[String], outer: &[String]) -> bool {
    inner.len() > outer.len() && inner.starts_with(outer)
}

/// 展开出的内容里，`path` 处的 `$defs` 是否删掉：它所在的 Schema 根严格在片段内部时保留。
fn drops_defs(fragment: Option<&[String]>, path: &[String]) -> bool {
    fragment.is_some_and(|fragment| !strictly_inside(path, fragment))
}

/// 按 Pointer 的各段往下走，只经过对象的键。
fn descend<'a>(root: &'a Json, path: &[String]) -> Option<&'a Json> {
    path.iter()
        .try_fold(root, |node, segment| node.as_object()?.get(segment))
}

/// JSON Pointer 拆成段；空串是根。
fn segments(pointer: &str) -> Option<Vec<String>> {
    if pointer.is_empty() {
        return Some(Vec::new());
    }
    let rest = pointer.strip_prefix('/')?;
    Some(
        rest.split('/')
            .map(|s| s.replace("~1", "/").replace("~0", "~"))
            .collect(),
    )
}

fn pointer(path: &[String]) -> String {
    if path.is_empty() {
        return "（根）".to_owned();
    }
    path.iter()
        .map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1")))
        .collect()
}

fn key(block: &str, path: &[String]) -> String {
    format!("{block}#{}", pointer(path))
}

fn child(path: &[String], segment: &str) -> Vec<String> {
    let mut out = path.to_vec();
    out.push(segment.to_owned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    /// 每个 (名字, JSON) 写成一个 block，按名字索引。
    fn docs(entries: &[(&str, Json)]) -> BTreeMap<String, Block> {
        entries
            .iter()
            .enumerate()
            .map(|(i, (name, value))| {
                let block = Block {
                    name: (*name).to_owned(),
                    lang: "json".to_owned(),
                    text: value.to_string(),
                    file: "t.md".to_owned(),
                    line: i + 1,
                };
                ((*name).to_owned(), block)
            })
            .collect()
    }

    fn expanded(entries: &[(&str, Json)], name: &str) -> Json {
        let out = expand(&docs(entries)).unwrap_or_else(|e| panic!("应当展开得了：{e:?}"));
        serde_json::from_str(&out[name].text).unwrap()
    }

    fn error(entries: &[(&str, Json)]) -> ExpandError {
        let errors = expand(&docs(entries)).expect_err("应当展开不了");
        errors.into_iter().next().unwrap()
    }

    fn source() -> (&'static str, Json) {
        (
            "src",
            json!({
                "in": {
                    "type": "object",
                    "properties": { "p": { "$ref": "#/$defs/n" } },
                    "$defs": { "n": { "type": "string", "description": "n", "items": { "$ref": "#/$defs/m" } }, "m": { "type": "integer" } }
                }
            }),
        )
    }

    #[test]
    fn refs_in_the_expanded_content_resolve_against_their_own_schema_and_the_hosts_stay() {
        let host = json!({
            "anyOf": [{ "$block": "src#/in" }, { "$ref": "#/$defs/h" }],
            "$defs": { "h": { "type": "null" } }
        });
        let got = expanded(&[source(), ("host", host)], "host");
        assert_eq!(
            got,
            json!({
                "anyOf": [
                    { "type": "object", "properties": { "p": { "type": "string", "description": "n", "items": { "type": "integer" } } } },
                    { "$ref": "#/$defs/h" }
                ],
                "$defs": { "h": { "type": "null" } }
            })
        );
    }

    #[test]
    fn a_self_contained_fragment_expands_to_its_original_text() {
        // 片段是一份 Port spec：in 是带 $defs 的 Schema 根，严格在片段内部，所以原样保留。
        let port = json!({
            "in": { "type": "object", "properties": { "m": { "$ref": "#/$defs/m" } }, "$defs": { "m": { "type": "string" } } },
            "out": { "value": { "type": "null" } }
        });
        let got = expanded(
            &[
                ("src", json!({ "p": port.clone() })),
                ("host", json!({ "x": { "$block": "src#/p" } })),
            ],
            "host",
        );
        assert_eq!(got, json!({ "x": port }));
    }

    #[test]
    fn a_kernel_port_spec_expands_to_its_text_in_reference_kernel() {
        let root = crate::generate::find_root(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("向上找到 tracked.toml 即仓库根");
        let mut blocks = crate::docs(&root).unwrap();
        let host = docs(&[("host", json!({ "$block": "reference.kernel#/pack" }))]);
        blocks.extend(host);
        let out = expand(&blocks).unwrap_or_else(|e| panic!("{e:?}"));
        let got: Json = serde_json::from_str(&out["host"].text).unwrap();
        let kernel: Json = serde_json::from_str(&out["reference.kernel"].text).unwrap();
        assert_eq!(got, kernel["pack"]);
        assert!(got["in"].is_object(), "Kernel Port 的 in 保持完整");
    }

    #[test]
    fn keys_beside_block_override_the_expanded_ones() {
        let host = json!({ "$block": "src#/in/$defs/n", "description": "own" });
        let got = expanded(&[source(), ("host", host)], "host");
        assert_eq!(
            got,
            json!({ "type": "string", "description": "own", "items": { "type": "integer" } })
        );
    }

    #[test]
    fn a_block_without_block_keeps_its_text() {
        let blocks = docs(&[source()]);
        assert_eq!(expand(&blocks).unwrap()["src"].text, blocks["src"].text);
    }

    #[test]
    fn unresolvable_names_pointers_and_refs_point_back_to_the_block() {
        let e = error(&[("host", json!({ "x": { "$block": "nope" } }))]);
        assert_eq!(
            (e.name.as_str(), e.file.as_str(), e.line, e.at.as_str()),
            ("host", "t.md", 1, "/x")
        );
        assert!(e.message.contains("没有名为 nope"), "{}", e.message);

        let e = error(&[source(), ("host", json!({ "$block": "src#/in/missing" }))]);
        assert!(e.message.contains("解析不到"), "{}", e.message);

        let e = error(&[
            (
                "src",
                json!({ "a": { "$ref": "#/$defs/gone" }, "$defs": {} }),
            ),
            ("host", json!({ "$block": "src#/a" })),
        ]);
        assert_eq!((e.name.as_str(), e.at.as_str()), ("src", "/a"));
        assert!(e.message.contains("找不到"), "{}", e.message);
    }

    #[test]
    fn keys_beside_block_need_an_object_to_override() {
        let e = error(&[
            source(),
            (
                "host",
                json!({ "$block": "src#/in/properties/p/$ref", "description": "x" }),
            ),
        ]);
        assert!(e.message.contains("不是对象"), "{}", e.message);
    }

    #[test]
    fn cycles_are_reported_instead_of_looping() {
        let e = error(&[
            ("a", json!({ "$block": "b" })),
            ("b", json!({ "x": { "$block": "a" } })),
        ]);
        assert!(e.message.contains("成环"), "{}", e.message);

        let e = error(&[("a", json!({ "x": { "$block": "a#/x" } }))]);
        assert!(e.message.contains("成环"), "{}", e.message);

        let tree = json!({ "s": { "$ref": "#/$defs/t" }, "$defs": { "t": { "items": { "$ref": "#/$defs/t" } } } });
        let e = error(&[("r", tree), ("host", json!({ "$block": "r#/s" }))]);
        assert!(e.message.contains("递归"), "{}", e.message);
    }

    #[test]
    fn every_block_in_the_repository_expands() {
        let root = crate::generate::find_root(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("向上找到 tracked.toml 即仓库根");
        let out = expand(&crate::docs(&root).unwrap()).unwrap_or_else(|e| panic!("{e:?}"));
        for block in out.values().filter(|b| b.lang == "json") {
            assert!(
                !block.text.contains("\"$block\""),
                "{} 里还有 $block",
                block.name
            );
        }
    }
}
