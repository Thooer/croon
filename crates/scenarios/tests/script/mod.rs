//! 测试用的 Language `script@1`：body 是一列 Atom，按顺序报出，前一个有了结果才报下一个，最后给出 done。
//! 它代替还没有的真的求值器，用来检验脚手架的「真的 Language」：Atom 由它报出，脚手架管不着。
//! 只在测试里登记，不进 Kernel。
//! 参考：docs/Kernel/Scenarios/README.md（脚手架）；docs/Kernel/Definition/Session.md（会话）。
//!
//! Atom 里要用之前结果的地方写成 `{"$result": 键, "at": JSON Pointer}`，报出前换成那个结果里的值；
//! 只在目标、附带与取的 filter 里出现，交出的值原样报出。

use std::collections::{BTreeMap, BTreeSet};

use kernel::host::{CheckError, HostError, Language, Read, Session};
use kernel::ids::{IdKind, kind_of};
use kernel::index::what;
use scenarios::scenario::Scn;
use serde_json::{Value as Json, json};

pub struct Script;

struct ScriptSession {
    steps: Vec<Json>,
    done: Option<Json>,
    results: BTreeMap<String, Json>,
    reported: BTreeSet<String>,
}

impl Language for Script {
    fn id(&self) -> &str {
        "script@1"
    }
    fn check(&self, _: &Json, _: &dyn Read) -> Result<(), CheckError> {
        Ok(())
    }
    fn open(
        &self,
        _: &str,
        birth: &Json,
        results: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        let body = &birth["definition"]["body"];
        Ok(Box::new(ScriptSession {
            steps: body["steps"].as_array().cloned().unwrap_or_default(),
            done: body.get("done").cloned(),
            results: results.clone(),
            reported: BTreeSet::new(),
        }))
    }
}

impl ScriptSession {
    fn resolve(&self, v: &Json) -> Json {
        match v {
            Json::Object(m) if m.contains_key("$result") => {
                let key = m["$result"].as_str().unwrap();
                let at = m["at"].as_str().unwrap();
                self.results[key]
                    .pointer(at)
                    .unwrap_or_else(|| panic!("结果 {key} 里没有 {at}"))
                    .clone()
            }
            Json::Object(m) => Json::Object(
                m.iter()
                    .map(|(k, x)| (k.clone(), self.resolve(x)))
                    .collect(),
            ),
            Json::Array(a) => Json::Array(a.iter().map(|x| self.resolve(x)).collect()),
            other => other.clone(),
        }
    }
}

impl Session for ScriptSession {
    fn resume(&mut self, new: &BTreeMap<String, Json>, _: &dyn Read) -> Result<Json, HostError> {
        self.results
            .extend(new.iter().map(|(k, v)| (k.clone(), v.clone())));
        for step in &self.steps {
            let key = step["key"].as_str().unwrap();
            if self.results.contains_key(key) {
                continue;
            }
            if !self.reported.insert(key.to_owned()) {
                return Ok(json!({"atoms": []}));
            }
            let mut atom = step.clone();
            if let Some(send) = atom.get_mut("send") {
                send["to"] = self.resolve(&send["to"]);
                send["attach"] = self.resolve(&send["attach"]);
            } else {
                atom["receive"] = self.resolve(&atom["receive"]);
            }
            return Ok(json!({"atoms": [atom]}));
        }
        Ok(match &self.done {
            Some(done) => json!({"done": done}),
            None => json!({"atoms": []}),
        })
    }
}

/// JSON Pointer 的一节：`~` 写成 `~0`，`/` 写成 `~1`。
fn token(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// 一本预期的 Journal 里，Handle 的占位名 `h` 在它之前从哪个结果进来：写成 `$result`。
fn origin(journal: &[Json], h: &Json) -> Json {
    for r in journal {
        let (kind, body) = what(r);
        if kind != "send" {
            continue;
        }
        let key = &body["key"];
        let result = &body["result"];
        for (name, i) in result["attach"].as_object().into_iter().flatten() {
            if i["handle"] == *h {
                return json!({"$result": key, "at": format!("/attach/{}/handle", token(name))});
            }
        }
        for (name, d) in result["derived"].as_object().into_iter().flatten() {
            if d["handle"] == *h {
                return json!({"$result": key, "at": format!("/derived/{}/handle", token(name))});
            }
            if d["revoker"]["handle"] == *h {
                return json!({"$result": key, "at": format!("/derived/{}/revoker/handle", token(name))});
            }
        }
    }
    for (seq, r) in journal.iter().enumerate() {
        let (kind, body) = what(r);
        if kind != "arrival" {
            continue;
        }
        for (name, i) in body["attach"].as_object().into_iter().flatten() {
            if i["handle"] == *h {
                let take = journal
                    .iter()
                    .find(|t| what(t).0 == "receive" && what(t).1["result"] == seq)
                    .unwrap_or_else(|| panic!("带来 {h} 的到达 {seq} 没有被取"));
                return json!({"$result": what(take).1["key"], "at": format!("/arrival/attach/{}/handle", token(name))});
            }
        }
    }
    panic!("{h} 不是从之前的结果进来的");
}

/// 目标、附带、filter 里的 Handle 占位名换成 `$result`。
fn refs(journal: &[Json], v: &Json) -> Json {
    match v {
        Json::String(s) if kind_of(s) == Some(IdKind::Handle) => origin(journal, v),
        Json::Object(m) => Json::Object(
            m.iter()
                .map(|(k, x)| (k.clone(), refs(journal, x)))
                .collect(),
        ),
        Json::Array(a) => Json::Array(a.iter().map(|x| refs(journal, x)).collect()),
        other => other.clone(),
    }
}

/// 一本预期的 Journal 里自己的步，写成 script 的 Atom；`pending` 接在后面。
/// 一份 script 的 body：按顺序的 Atom，与 done 的结果（Journal 里没有结束的，没有 done）。
type Body = (Vec<Json>, Option<Json>);

fn value_from(values: &BTreeMap<String, Json>, value: &Json) -> Json {
    let Some(id) = value.as_str() else {
        return value.clone();
    };
    let Some(content) = values.get(id) else {
        return value.clone();
    };
    if let Some(alias) = content.get("$alias").and_then(Json::as_str) {
        return value_from(values, &Json::String(alias.to_owned()));
    }
    content.clone()
}

fn steps(journal: &[Json], values: &BTreeMap<String, Json>, pending: &[Json]) -> Body {
    let mut steps = Vec::new();
    let mut done = None;
    for r in journal.iter().filter(|r| r["by"] == "self") {
        let (kind, body) = what(r);
        match kind {
            "send" => {
                let attach: serde_json::Map<String, Json> = body["attach"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(n, a)| (n.clone(), refs(journal, &a["to"])))
                    .collect();
                let value = value_from(values, &body["delivery"]["value"]);
                let mut dependencies = body["delivery"]["dependencies"].clone();
                if let Some(items) = dependencies.as_array_mut() {
                    items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                    items.dedup();
                }
                steps.push(json!({"key": body["key"], "send": {"to": refs(journal, &body["to"]), "delivery": {"value":value,"dependencies":dependencies}, "attach": attach}}));
            }
            "receive" => {
                steps.push(json!({"key": body["key"], "receive": refs(journal, &body["filter"])}))
            }
            "end" => done = Some(body.clone()),
            _ => {}
        }
    }
    steps.extend(pending.iter().cloned());
    (steps, done)
}

fn replace(v: &mut Json, old: &Json, new: &Json) {
    if v == old {
        *v = new.clone();
        return;
    }
    match v {
        Json::Object(m) => m.values_mut().for_each(|x| replace(x, old, new)),
        Json::Array(a) => a.iter_mut().for_each(|x| replace(x, old, new)),
        _ => {}
    }
}

/// 这个场景里 `fake@1` 的 Definition 都换成 `script@1`，body 由用它的 Instance 的预期 Journal 写出：
/// 几个 Instance 用同一份 Definition 的，取步最多的那本，其余的要是它的前缀。原来的 Definition 出现在
/// 别的 Value 里（如构造用的那份）的，一并换掉。返回换过的 Value 名字空间。
pub fn scripted(scn: &Scn, values: &BTreeMap<String, Json>) -> BTreeMap<String, Json> {
    let mut by_def: BTreeMap<String, Vec<Body>> = BTreeMap::new();
    for (ph, journal) in &scn.journals {
        let d = what(&journal[0]).1["definition"].as_str().unwrap();
        if values[d]["$bundle"]["language"] != "fake@1" {
            continue;
        }
        let pending = scn.pending.get(ph).cloned().unwrap_or_default();
        by_def
            .entry(d.to_owned())
            .or_default()
            .push(steps(journal, values, &pending));
    }
    let mut out = values.clone();
    for (d, mut scripts) in by_def {
        scripts.sort_by_key(|(s, done)| s.len() + usize::from(done.is_some()));
        let (longest, done) = scripts.pop().unwrap();
        let old = values[&d].clone();
        let mut new = old.clone();
        new["$bundle"]["language"] = json!("script@1");
        new["$bundle"]["body"] = match done {
            Some(done) => json!({"steps": longest, "done": done}),
            None => json!({"steps": longest}),
        };
        for v in out.values_mut() {
            replace(v, &old, &new);
        }
    }
    out
}
