//! 读文档里的场景：每个场景一个 json block，名字是 `scenario.` 加文件名，Language 的场景前面再加
//! Language 的名字，如 `lang0.scenario.delegate`。所有场景共用一个 Value 的名字空间。
//! 参考：docs/Kernel/Scenarios/README.md（格式）。

use std::collections::BTreeMap;

use crate::blocks::{self, BLOCKS};
use serde_json::{Value as Json, json};

/// (Instance 的占位名, 序号)。
pub type Step = (String, usize);

pub struct Scn {
    pub name: String,
    /// Instance 的占位名 → 预期的 Journal，补上了 seq。
    pub journals: BTreeMap<String, Vec<Json>>,
    pub pending: BTreeMap<String, Vec<Json>>,
    pub order: Vec<(Step, Step)>,
    pub perturb: Vec<Json>,
}

/// 是不是一个场景的 block：Kernel 的写 `scenario.名字`，Language 的写 `语言.scenario.名字`。
pub fn is_scenario(name: &str) -> bool {
    name.starts_with("scenario.")
        || name
            .split_once('.')
            .is_some_and(|(_, rest)| rest.starts_with("scenario."))
}

/// 文档里全部场景：名字与展开过 `$block` 的内容。
pub fn all() -> Vec<(&'static str, Json)> {
    BLOCKS
        .iter()
        .filter(|b| b.lang == "json" && is_scenario(b.name))
        .map(|b| (b.name, blocks::json(b.name)))
        .collect()
}

/// 元素是数组的就地展平。
pub fn flatten(node: &Json, out: &mut Vec<Json>) {
    match node {
        Json::Array(items) => items.iter().for_each(|item| flatten(item, out)),
        other => out.push(other.clone()),
    }
}

/// 共用的 Value 名字空间：名字 → 内容，还没展开 `$file` 这些。同一个名字只写一处。
pub fn values(all: &[(&str, Json)]) -> BTreeMap<String, Json> {
    let mut out = BTreeMap::new();
    for (name, scenario) in all {
        for (id, content) in scenario
            .get("values")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            assert!(
                out.insert(id.clone(), content.clone()).is_none(),
                "{name}：Value {id} 在别的场景里已经写过"
            );
        }
    }
    out
}

/// 一个场景的 Journal：Instance → 按序号排的记录，补上 seq。
pub fn journals(scenario: &Json) -> BTreeMap<String, Vec<Json>> {
    scenario["journals"]
        .as_object()
        .expect("journals 是对象")
        .iter()
        .map(|(instance, records)| {
            let mut flat = Vec::new();
            flatten(records, &mut flat);
            for (seq, record) in flat.iter_mut().enumerate() {
                record["seq"] = json!(seq);
            }
            (instance.clone(), flat)
        })
        .collect()
}

impl Scn {
    pub fn of(name: &str, scenario: &Json) -> Scn {
        let pending = scenario
            .get("pending")
            .and_then(Json::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_array().unwrap().clone()))
                    .collect()
            })
            .unwrap_or_default();
        let step = |v: &Json| {
            (
                v[0].as_str().unwrap().to_owned(),
                v[1].as_u64().unwrap() as usize,
            )
        };
        let order = scenario
            .get("order")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .map(|o| (step(&o["first"]), step(&o["then"])))
            .collect();
        let perturb = scenario
            .get("perturb")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        Scn {
            name: name.to_owned(),
            journals: journals(scenario),
            pending,
            order,
            perturb,
        }
    }
}

/// Kernel 的场景及其共用 Value；不展开具体 Language 的 Library。
pub fn kernel() -> (Vec<Scn>, BTreeMap<String, Json>) {
    let all: Vec<_> = all()
        .into_iter()
        .filter(|(name, _)| name.starts_with("scenario."))
        .collect();
    let values = values(&all);
    let scns: Vec<Scn> = all.iter().map(|(n, s)| Scn::of(n, s)).collect();
    assert!(
        scns.len() >= 6,
        "文档里应当有首刀的六个场景，实际 {}",
        scns.len()
    );
    (scns, values)
}
