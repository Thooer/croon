//! 假的只接在三处：Language 的插口（`fake@1`）、模型的提供者、exec 的容器。
//! Definition 写着 `fake@1` 的 Instance，Atom 由脚手架按它的 Journal 报出；别的 Language 的 Instance，
//! Atom 由那种 Language 报出，脚手架只决定假的 llm 与 exec 何时给出结果。
//! 参考：docs/Kernel/Scenarios/README.md（脚手架）。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use kernel::host::{
    Call, Channel, CheckError, Execution, HostDone, HostError, Language, Port, Read, Session,
};
use kernel::index::what;
use kernel::store::Content;
use serde_json::{Value as Json, json};

use crate::bind::Bind;
use crate::scenario::{Scn, Step};

pub struct Shared {
    pub scn: Rc<Scn>,
    pub bind: Bind,
    /// 现在允许走的那一步：(Instance 的占位名, 序号)。
    pub allowed: Option<Step>,
    /// Definition 不是 `fake@1` 的、求值器在宿主的 Instance 的占位名：真的 Language。
    pub real: BTreeSet<String>,
}

impl Shared {
    /// 预期里这个 Instance 的这个键的交出记录：(占位名, 序号, 记录的 send)。
    fn expected_send(&self, instance: &str, key: &str) -> Option<(String, usize, Json)> {
        let ph = self.bind.back.get(instance)?;
        self.scn.journals[ph].iter().enumerate().find_map(|(n, r)| {
            let (kind, body) = what(r);
            (kind == "send" && body["key"] == key).then(|| (ph.clone(), n, body.clone()))
        })
    }

    /// 交给 llm 或 exec 的这一次能不能给出结果：`fake@1` 的随时给，它的先后由报出决定；
    /// 真的 Language 的只在这一步被允许时给。
    fn may_answer(&self, ph: &str, n: usize) -> bool {
        !self.real.contains(ph) || self.allowed == Some((ph.to_owned(), n))
    }
}

/// 一条预期记录对应的 Atom：交出给出 key、to、值的内容与附带的目标，取给出 key 与 filter。
pub fn atom_of(bind: &mut Bind, record: &Json) -> Json {
    let (kind, body) = what(record);
    if kind == "send" {
        let mut attach = serde_json::Map::new();
        for (name, a) in body["attach"].as_object().unwrap() {
            attach.insert(name.clone(), bind.subst(&a["to"]));
        }
        let value = if let Some(id) = body["delivery"]["value"].as_str() {
            bind.content(id)
        } else {
            bind.subst(&body["delivery"]["value"])
        };
        let mut dependencies = bind.subst(&body["delivery"]["dependencies"]);
        if let Some(items) = dependencies.as_array_mut() {
            items.sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
            items.dedup();
        }
        let mut atom = json!({"key": body["key"], "send": {"to": bind.subst(&body["to"]), "delivery": {"value":value,"dependencies":dependencies}, "attach": attach}});
        sort_dependencies(&mut atom["send"]["delivery"]["dependencies"]);
        atom
    } else {
        json!({"key": body["key"], "receive": bind.subst(&body["filter"])})
    }
}

pub struct FakeLang(pub Rc<RefCell<Shared>>);

struct FakeSession {
    shared: Rc<RefCell<Shared>>,
    instance: String,
    results: BTreeSet<String>,
    reported: BTreeSet<String>,
    pending_reported: bool,
}

impl Language for FakeLang {
    fn id(&self) -> &str {
        "fake@1"
    }
    fn check(&self, _: &Json, _: &dyn Read) -> Result<(), CheckError> {
        Ok(())
    }
    fn open(
        &self,
        instance: &str,
        _birth: &Json,
        results: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        Ok(Box::new(FakeSession {
            shared: self.0.clone(),
            instance: instance.to_owned(),
            results: results.keys().cloned().collect(),
            reported: BTreeSet::new(),
            pending_reported: false,
        }))
    }
}

impl Session for FakeSession {
    fn resume(&mut self, new: &BTreeMap<String, Json>, _: &dyn Read) -> Result<Json, HostError> {
        self.results.extend(new.keys().cloned());
        let mut s = self.shared.borrow_mut();
        let Some(ph) = s.bind.back.get(&self.instance).cloned() else {
            return Ok(json!({"atoms": []}));
        };
        let script = s.scn.journals[&ph].clone();
        for (idx, r) in script.iter().enumerate() {
            if r["by"] != "self" {
                continue;
            }
            let (kind, body) = what(r);
            match kind {
                "send" | "receive" => {
                    let key = body["key"].as_str().unwrap();
                    if self.results.contains(key) {
                        continue;
                    }
                    if self.reported.contains(key) || s.allowed != Some((ph.clone(), idx)) {
                        return Ok(json!({"atoms": []}));
                    }
                    self.reported.insert(key.to_owned());
                    return Ok(json!({"atoms": [atom_of(&mut s.bind, r)]}));
                }
                "end" => {
                    return Ok(if s.allowed == Some((ph, idx)) {
                        json!({"done": s.bind.subst(body)})
                    } else {
                        json!({"atoms": []})
                    });
                }
                _ => {}
            }
        }
        if self.pending_reported {
            return Ok(json!({"atoms": []}));
        }
        self.pending_reported = true;
        let pending = s.scn.pending.get(&ph).cloned().unwrap_or_default();
        let atoms: Vec<Json> = pending
            .iter()
            .map(|a| pending_atom(&mut s.bind, a))
            .collect();
        Ok(json!({ "atoms": atoms }))
    }
}

/// 宿主装配按源 Handle 指定用途；每次 start 产生独立的收取对象。
pub struct FakePort {
    pub shared: Rc<RefCell<Shared>>,
    pub channel: Channel,
    pub identity: String,
}
struct FakeExecution {
    shared: Rc<RefCell<Shared>>,
    instance: String,
    key: String,
    content: Rc<Content>,
}
impl Port for FakePort {
    fn id(&self) -> &str {
        &self.identity
    }
    fn channel(&self) -> Channel {
        self.channel
    }
    fn start(&self, call: &Call) -> Result<Box<dyn Execution>, HostError> {
        Ok(Box::new(FakeExecution {
            shared: self.shared.clone(),
            instance: call.instance.into(),
            key: call.key.into(),
            content: call.content.clone(),
        }))
    }
    fn retire(&self, _: &str) -> Result<(), HostError> {
        Ok(())
    }
}
impl Execution for FakeExecution {
    fn collect(&mut self) -> Result<Option<HostDone>, HostError> {
        let mut s = self.shared.borrow_mut();
        let Some((ph, n, body)) = s.expected_send(&self.instance, &self.key) else {
            return Ok(None);
        };
        if !s.may_answer(&ph, n) {
            return Ok(None);
        }
        let outcome = s.bind.subst(&body["result"]["outcome"]);
        let roots = outcome["dependencies"].as_array().unwrap();
        for id in roots {
            make_bundle(&mut s.bind, &self.content, id.as_str().unwrap())?;
        }
        Ok(Some(HostDone { outcome }))
    }
}

fn pending_atom(bind: &mut Bind, raw: &Json) -> Json {
    if raw.get("send").is_none() {
        return bind.subst(raw);
    }
    let send = &raw["send"];
    let value = if let Some(id) = send["delivery"]["value"].as_str() {
        bind.content(id)
    } else {
        bind.subst(&send["delivery"]["value"])
    };
    let attach = send["attach"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(n, t)| (n.clone(), bind.subst(t)))
        .collect::<serde_json::Map<_, _>>();
    let mut atom = json!({"key":raw["key"],"send":{"to":bind.subst(&send["to"]),"delivery":{"value":value,"dependencies":bind.subst(&send["delivery"]["dependencies"])},"attach":attach}});
    sort_dependencies(&mut atom["send"]["delivery"]["dependencies"]);
    atom
}
fn sort_dependencies(v: &mut Json) {
    if let Some(a) = v.as_array_mut() {
        a.sort_by(|x, y| x.as_str().unwrap().cmp(y.as_str().unwrap()));
        a.dedup();
    }
}

fn make_bundle(bind: &mut Bind, content: &Content, id: &str) -> Result<(), HostError> {
    if content.bundle(id)?.is_some() {
        return Ok(());
    }
    let bundle = bind
        .made
        .get(id)
        .cloned()
        .ok_or_else(|| HostError(format!("产物Bundle {id} 未在规格声明")))?;
    let dependencies: Vec<String> = bundle["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    for dep in &dependencies {
        make_bundle(bind, content, dep)?;
    }
    let raw = bundle["content"].as_str().unwrap();
    let actual = if let Some(value) = bind.made.get(raw) {
        content.put_json(value)?
    } else if let Some(bytes) = bind.bytes.get(raw) {
        content.put_bytes(bytes)?
    } else {
        return Err(HostError(format!("产物内容 {raw} 未声明")));
    };
    assert_eq!(actual, raw);
    assert_eq!(content.pack(raw, &dependencies)?, id);
    Ok(())
}
