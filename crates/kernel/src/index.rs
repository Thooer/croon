//! 索引：从各本 Journal 推出的全系统状态。
//! 参考：docs/Kernel/Journal.md（恢复就是重放、实现）；docs/Kernel/Reference.md（获得、撤销、限定、能读的 ID）；docs/Kernel/Definition/README.md（Interface 与 Implementation）。
//!
//! 一条记录添进索引的，只取决于它自己与同一本 Journal 里在它之前的记录：`apply` 只写这本
//! Journal 的表，与按 Handle、事件 ID 为键、每个键只由一条记录写下的表。要用到别的 Journal
//! 的关系（Handle 指向谁、派生链、能读的集合）只存引用，查询时才解开，见 `View`。所以各本
//! Journal 按什么次序重建，得到的索引都相同。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::{Value as Json, json};

use croon_eval::value::ValueId;
use croon_port::{ContentStore, Scope};

/// 一条记录的位置：(Journal, 序号)。
pub type Pos = (String, u64);

/// 一个 Handle 是怎样来的。每个 Handle 只由一条记录产生。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mint {
    /// 交出时为附带 `name` 派生出的那一份；父是那次交出里 `attach[name].ref`。
    Derived { at: Pos, name: String },
    /// 那一份的撤销权。
    Revoker { at: Pos, name: String },
    /// 交出结果带来的：instantiate 给出的 `port/<名字>`、look、notify、correct。
    Result { at: Pos, name: String },
}

/// 一个 Reference 所指的 Port。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    /// Kernel 的操作：pack、construct、instantiate。
    Kernel(String),
    /// 宿主源由根诞生的 wire Handle 标识。
    Host(String),
    /// 一个 Instance 的 Provided port。
    Provided {
        instance: String,
        port: String,
    },
    Look(String),
    Notify(String),
    Correct(String),
    /// 撤销这一份派生出的 Reference。
    Revoke(String),
}

/// 派生链上的一节：一份 Port spec 的 ID 加 JSON Pointer 写的位置。`author` 是写下它的
/// Instance；提供者是 Kernel 的那一节没有作者。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub spec: String,
    pub ptr: String,
    pub author: Option<String>,
}

/// 一本 Journal 与只由它推出的表。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Journal {
    pub records: Vec<Json>,
    /// 自己的一步：键 → 序号。
    pub keys: BTreeMap<String, u64>,
    /// 诞生里由系统外造成：root entry。
    pub root: bool,
    /// Required port → 接着的 Handle（诞生、授予、收回）。
    pub wire: BTreeMap<String, String>,
    /// 交出的结果与到达里新得到的 Handle → 得到时的序号。
    pub acquired: BTreeMap<String, u64>,
    /// 每个 Handle 第一次得到时的序号，失效的排序用它。
    pub since: BTreeMap<String, u64>,
    pub invalid: BTreeSet<String>,
    /// 还没取的到达的序号。
    pub untaken: BTreeSet<u64>,
    pub paused: bool,
    pub ended: Option<u64>,
}

impl Journal {
    /// 持有它：接在某个 Required port 上，或者交出的结果与到达里得到过，且没有失效。
    pub fn holds(&self, handle: &str) -> bool {
        !self.invalid.contains(handle)
            && (self.acquired.contains_key(handle) || self.wire.values().any(|h| h == handle))
    }

    pub fn birth(&self) -> &Json {
        &self.records[0]["what"]["birth"]
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Index {
    pub journals: BTreeMap<String, Journal>,
    pub mints: BTreeMap<String, Mint>,
    /// 诞生里接上的 Handle → (Instance, Required port)。派生出的也在这里，查 `mints` 优先。
    pub birth_wire: BTreeMap<String, (String, String)>,
    /// 事件 → 造成它的那一步。
    pub events: BTreeMap<String, Pos>,
    /// 实例化那一步的事件 → 生出的 Instance。
    pub born_by: BTreeMap<String, String>,
    /// 成功的交出：经哪个 Handle → 位置。撤销与 Notify 的登记从这里查。
    pub ok_sends: BTreeMap<String, BTreeSet<Pos>>,
}

/// 一条记录的种类与内容。
pub fn what(record: &Json) -> (&str, &Json) {
    let map = record["what"].as_object().expect("记录的 what 是对象");
    let (kind, body) = map.iter().next().expect("what 恰好一种");
    (kind.as_str(), body)
}

impl Index {
    pub fn rebuild(all: Vec<(String, Vec<Json>)>) -> Index {
        let mut index = Index::default();
        for (instance, records) in all {
            for record in records {
                index.apply(&instance, record);
            }
        }
        index
    }

    /// 把一条已提交的记录添进索引。写记录与重建都只走这一处。
    pub fn apply(&mut self, instance: &str, record: Json) {
        let seq = record["seq"].as_u64().expect("记录有序号");
        let at: Pos = (instance.to_owned(), seq);
        let j = self.journals.entry(instance.to_owned()).or_default();
        assert_eq!(
            seq,
            j.records.len() as u64,
            "{instance} 的记录要按序号添进索引"
        );
        let (kind, body) = what(&record);
        let body = body.clone();
        if let Some(event) = record.get("event").and_then(Json::as_str)
            && record["by"] == "self"
            && matches!(kind, "send" | "end")
        {
            self.events.insert(event.to_owned(), at.clone());
        }
        match kind {
            "birth" => {
                j.root = record["by"] == "outside";
                for (port, h) in body["wire"].as_object().into_iter().flatten() {
                    let h = h.as_str().unwrap_or_default().to_owned();
                    j.wire.insert(port.clone(), h.clone());
                    j.since.entry(h.clone()).or_insert(seq);
                    self.birth_wire
                        .insert(h, (instance.to_owned(), port.clone()));
                }
                if let Some(event) = record.get("event").and_then(Json::as_str) {
                    self.born_by.insert(event.to_owned(), instance.to_owned());
                }
            }
            "send" => {
                j.keys.insert(str_of(&body["key"]), seq);
                let result = &body["result"];
                for (name, inc) in result["attach"].as_object().into_iter().flatten() {
                    let h = str_of(&inc["handle"]);
                    j.acquired.insert(h.clone(), seq);
                    j.since.entry(h.clone()).or_insert(seq);
                    self.mints.insert(
                        h,
                        Mint::Result {
                            at: at.clone(),
                            name: name.clone(),
                        },
                    );
                }
                for (name, d) in result["derived"].as_object().into_iter().flatten() {
                    let h = str_of(&d["handle"]);
                    let r = str_of(&d["revoker"]["handle"]);
                    j.acquired.insert(r.clone(), seq);
                    j.since.entry(r.clone()).or_insert(seq);
                    self.mints.insert(
                        h,
                        Mint::Derived {
                            at: at.clone(),
                            name: name.clone(),
                        },
                    );
                    self.mints.insert(
                        r,
                        Mint::Revoker {
                            at: at.clone(),
                            name: name.clone(),
                        },
                    );
                }
                if result["outcome"]["ok"] == true
                    && let Some(r) = body["ref"].as_str()
                {
                    self.ok_sends
                        .entry(r.to_owned())
                        .or_default()
                        .insert(at.clone());
                }
            }
            "receive" => {
                j.keys.insert(str_of(&body["key"]), seq);
                if let Some(n) = body["result"].as_u64() {
                    j.untaken.remove(&n);
                }
            }
            "arrival" => {
                j.untaken.insert(seq);
                for inc in body["attach"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(_, v)| v)
                {
                    let h = str_of(&inc["handle"]);
                    j.acquired.insert(h.clone(), seq);
                    j.since.entry(h).or_insert(seq);
                }
            }
            "correct" => match body["op"].as_str() {
                Some("pause") => j.paused = true,
                Some("resume") => j.paused = false,
                Some("grant") => {
                    let h = str_of(&body["ref"]);
                    j.since.entry(h.clone()).or_insert(seq);
                    j.wire.insert(str_of(&body["port"]), h);
                }
                Some("withdraw") => {
                    j.wire.remove(body["port"].as_str().unwrap_or_default());
                }
                _ => {}
            },
            "invalid" => {
                for h in body.as_array().into_iter().flatten() {
                    j.invalid.insert(str_of(h));
                }
            }
            "end" => j.ended = Some(seq),
            _ => {}
        }
        j.records.push(record);
    }

    pub fn journal(&self, instance: &str) -> Option<&Journal> {
        self.journals.get(instance)
    }

    pub fn record(&self, at: &Pos) -> &Json {
        &self.journals[&at.0].records[at.1 as usize]
    }

    /// root entry 的身份。
    pub fn root(&self) -> Option<&str> {
        self.journals
            .iter()
            .find(|(_, j)| j.root)
            .map(|(i, _)| i.as_str())
    }
}

fn str_of(v: &Json) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// reference.kernel 里一个 Port 的 Port spec 与它的 ID。
pub fn kernel_spec(name: &str) -> (&'static Json, &'static ValueId) {
    static SPECS: OnceLock<BTreeMap<String, (Json, ValueId)>> = OnceLock::new();
    let specs = SPECS.get_or_init(|| {
        let registry = crate::blocks::json("reference.kernel");
        registry
            .as_object()
            .expect("reference.kernel 是对象")
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    (v.clone(), ValueId::of_json(v).expect("Port spec 是 Value")),
                )
            })
            .collect()
    });
    let (spec, id) = specs
        .get(name)
        .unwrap_or_else(|| panic!("reference.kernel 里没有 {name}"));
    (spec, id)
}

pub fn kernel_ports() -> Vec<&'static str> {
    vec![
        "pack",
        "construct",
        "instantiate",
        "look",
        "notify",
        "correct",
        "revoke",
    ]
}

/// 去掉各处的 description，定义在 croon-eval：Charter 的检查与这里用同一条。
pub use croon_eval::port::strip;

/// Port 这一层的 `$ref`：顶层与附带声明里的。in 与 out.value 是自身完整的 Schema，它们的 `$ref`
/// 按自己的 `$defs` 解析，与所在的 Port spec 无关。
fn has_ref(port: &Json) -> bool {
    let Some(m) = port.as_object() else {
        return false;
    };
    if m.contains_key("$ref") {
        return true;
    }
    let attach = |a: Option<&Json>| {
        a.is_some_and(|a| {
            a.get("properties")
                .and_then(Json::as_object)
                .is_some_and(|p| p.values().any(has_ref))
                || a.get("additionalProperties").is_some_and(has_ref)
        })
    };
    attach(m.get("attach"))
        || attach(m.get("out").and_then(|o| o.get("attach")))
        || m.get("$defs")
            .and_then(Json::as_object)
            .is_some_and(|d| d.values().any(has_ref))
}

/// 两份 Port spec 相同：去掉各处的 description 之后规范文本相同；含 `$ref` 的连同顶层一起比，
/// 位置也要相同。
pub fn same_spec(a_root: &Json, a_ptr: &str, b_root: &Json, b_ptr: &str) -> bool {
    let (Some(a), Some(b)) = (a_root.pointer(a_ptr), b_root.pointer(b_ptr)) else {
        return false;
    };
    if !has_ref(a) && !has_ref(b) {
        return croon_eval::schema::json_eq(&strip(a), &strip(b));
    }
    a_ptr == b_ptr && croon_eval::schema::json_eq(&strip(a_root), &strip(b_root))
}

fn esc(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// 索引上的查询：要用到内容库里的 Value（Definition、Port spec、交出的 Value）。
pub struct View<'a> {
    pub ix: &'a Index,
    pub store: &'a ContentStore,
}

impl View<'_> {
    pub fn load(&self, id: &str) -> Option<Json> {
        self.store.get_json(&ValueId::parse(id)?).ok().flatten()
    }

    /// 一个 Instance 的 Definition。
    pub fn definition(&self, instance: &str) -> Option<Json> {
        self.definition_bundle(self.ix.journal(instance)?.birth()["definition"].as_str()?)
    }

    pub fn definition_bundle(&self, id: &str) -> Option<Json> {
        let bundle = self.store.bundle(id).ok()??;
        self.load(&bundle.content)
    }

    /// Successful checks are global facts about the exact complete object.
    pub fn constructed(&self, id: &str) -> bool {
        self.ix
            .journals
            .values()
            .any(|j| j.root && j.birth()["definition"] == id)
            || self.ix.journals.values().any(|j| {
                j.records.iter().any(|record| {
                    let (kind, send) = what(record);
                    kind == "send"
                        && send["result"]["accepted"] == true
                        && send["result"]["outcome"]["ok"] == true
                        && send["result"]["outcome"]["value"]["id"] == id
                        && send["ref"].as_str().and_then(|r| self.provider(r))
                            == Some(Provider::Kernel("construct".into()))
                })
            })
    }

    /// 实例化它的那次交出的 Value；root entry 没有。
    fn spawn_value(&self, instance: &str) -> Option<Json> {
        let j = self.ix.journal(instance)?;
        let event = j.records[0].get("event")?.as_str()?;
        let at = self.ix.events.get(event)?;
        let (_, send) = what(self.ix.record(at));
        self.load(send["delivery"]["value"].as_str()?)
    }

    /// 用 self 接到自己的：`port/<名字>` 或 look。
    fn self_target(&self, instance: &str, port: &str) -> Option<String> {
        let j = self.ix.journal(instance)?;
        let s = if j.root {
            j.birth().get("self").cloned()
        } else {
            self.spawn_value(instance)?.get("self").cloned()
        };
        Some(s?.get(port)?.as_str()?.to_owned())
    }

    /// 诞生里用 self 接到自己的 Handle 所指的 Port。
    fn self_provider(instance: &str, target: &str) -> Option<Provider> {
        if target == "look" {
            return Some(Provider::Look(instance.to_owned()));
        }
        Some(Provider::Provided {
            instance: instance.to_owned(),
            port: target.strip_prefix("port/")?.to_owned(),
        })
    }

    pub fn ancestors(&self, handle: &str) -> Vec<String> {
        let mut out = vec![handle.to_owned()];
        let mut h = handle.to_owned();
        while let Some(Mint::Derived { at, name }) = self.ix.mints.get(&h) {
            let (_, send) = what(self.ix.record(at));
            let Some(parent) = send["attach"][name]["ref"].as_str() else {
                break;
            };
            out.push(parent.to_owned());
            h = parent.to_owned();
        }
        out
    }

    pub fn provider(&self, handle: &str) -> Option<Provider> {
        match self.ix.mints.get(handle) {
            Some(Mint::Derived { at, name }) => {
                let (_, send) = what(self.ix.record(at));
                self.provider(send["attach"][name]["ref"].as_str()?)
            }
            Some(Mint::Revoker { at, name }) => {
                let (_, send) = what(self.ix.record(at));
                Some(Provider::Revoke(
                    send["result"]["derived"][name]["handle"]
                        .as_str()?
                        .to_owned(),
                ))
            }
            Some(Mint::Result { at, name }) => {
                let born = self.ix.born_by.get(self.ix.record(at)["event"].as_str()?)?;
                Self::result_provider(born, name)
            }
            None => {
                let (instance, port) = self.ix.birth_wire.get(handle)?;
                let birth = self.ix.journal(instance)?.birth().clone();
                if let Some(collective) = birth.get("collective").and_then(Json::as_str) {
                    return self.charter_provider(collective, instance, &birth, port);
                }
                if let Some(target) = self.self_target(instance, port) {
                    return Self::self_provider(instance, &target);
                }
                // 派生出的已在 mints 里；其余的无来源 Handle 在 root entry 诞生中。
                let source = &self.ix.journal(instance)?.birth()["binding"]["sources"][port];
                if source["host"].is_string() {
                    Some(Provider::Host(handle.to_owned()))
                } else {
                    Some(Provider::Kernel(source["kernel"].as_str()?.to_owned()))
                }
            }
        }
    }

    fn result_provider(born: &str, name: &str) -> Option<Provider> {
        let born = born.to_owned();
        Some(match name {
            "look" => Provider::Look(born),
            "notify" => Provider::Notify(born),
            "correct" => Provider::Correct(born),
            other => Provider::Provided {
                instance: born,
                port: other.strip_prefix("port/")?.to_owned(),
            },
        })
    }

    /// 来源第一节固定准确规格；同一 Provider 可以通过不同契约取得。
    /// 成员的某个 Required port 在 Charter 里接的那一种来源，指向谁。与 `charter_source`
    /// 同一份数据；没有作者，所以只看是哪一种。
    fn charter_provider(
        &self,
        collective: &str,
        member: &str,
        birth: &Json,
        port: &str,
    ) -> Option<Provider> {
        let charter = self.definition(collective)?;
        let seat = birth["seat"].as_str()?;
        let source = charter["wire"].get(seat)?.get(port)?;
        if let Some(outer) = source.get("outer").and_then(Json::as_str) {
            let handle = self.ix.journal(collective)?.birth()["wire"].get(outer)?.as_str()?;
            return self.provider(handle);
        }
        if let Some(p) = source.get("self").and_then(Json::as_str) {
            return Some(Provider::Provided {
                instance: member.to_owned(),
                port: p.to_owned(),
            });
        }
        let other = match source.get("seat").and_then(Json::as_str) {
            Some(other) => self.member_of(collective, other)?,
            None => {
                let op = ["look", "notify", "correct"]
                    .into_iter()
                    .find(|op| source.get(op).is_some())?;
                let seat = source[op].as_str()?;
                return Some(match op {
                    "look" => Provider::Look(self.member_of(collective, seat)?),
                    "notify" => Provider::Notify(self.member_of(collective, seat)?),
                    _ => Provider::Correct(self.member_of(collective, seat)?),
                });
            }
        };
        Some(Provider::Provided {
            instance: other,
            port: source.get("port")?.as_str()?.to_owned(),
        })
    }

    pub fn source_spec(&self, handle: &str) -> Option<String> {
        Some(self.chain(handle)?.first()?.spec.clone())
    }

    fn result_spec(&self, at: &Pos, name: &str) -> Option<Node> {
        let (_, send) = what(self.ix.record(at));
        Some(Node {
            spec: ValueId::of_json(&send["result"]["attach"][name]["spec"])
                .ok()?
                .to_string(),
            ptr: String::new(),
            author: None,
        })
    }

    fn self_look_node(&self, instance: &str) -> Option<Node> {
        let j = self.ix.journal(instance)?;
        if j.root {
            return Some(Node {
                spec: kernel_spec("look").1.to_string(),
                ptr: String::new(),
                author: None,
            });
        }
        let at = self.ix.events.get(j.records[0]["event"].as_str()?)?;
        self.result_spec(at, "look")
    }

    pub fn spec_node_pub(def: &Json, side: &str, port: &str, author: &str) -> Option<Node> {
        Self::spec_node(def, side, port, author)
    }

    fn spec_node(def: &Json, side: &str, port: &str, author: &str) -> Option<Node> {
        let spec = def["interface"][side].get(port)?;
        Some(Node {
            spec: ValueId::of_json(spec).ok()?.to_string(),
            ptr: String::new(),
            author: Some(author.to_owned()),
        })
    }

    /// 一个 Handle 的派生链。
    pub fn chain(&self, handle: &str) -> Option<Vec<Node>> {
        match self.ix.mints.get(handle) {
            Some(Mint::Derived { at, name }) => {
                let (_, send) = what(self.ix.record(at));
                let parent = send["attach"][name]["ref"].as_str()?;
                let via = send["ref"].as_str()?;
                let value = self.load(send["delivery"]["value"].as_str()?)?;
                let born = self
                    .ix
                    .record(at)
                    .get("event")
                    .and_then(Json::as_str)
                    .and_then(|e| self.ix.born_by.get(e))
                    .cloned();
                let extra = self.receiver_node(via, &value, name, born.as_deref());
                self.derive_chain(parent, via, name, extra)
            }
            Some(Mint::Revoker { at, name }) => {
                let (_, send) = what(self.ix.record(at));
                Some(vec![Node {
                    spec: ValueId::of_json(&send["result"]["derived"][name]["revoker"]["spec"])
                        .ok()?
                        .to_string(),
                    ptr: String::new(),
                    author: None,
                }])
            }
            Some(Mint::Result { at, name }) => {
                let born = self.ix.born_by.get(self.ix.record(at)["event"].as_str()?)?;
                Some(match name.as_str() {
                    "look" | "notify" | "correct" => vec![self.result_spec(at, name)?],
                    other => {
                        let def = self.definition(born)?;
                        vec![Self::spec_node(
                            &def,
                            "provides",
                            other.strip_prefix("port/")?,
                            born,
                        )?]
                    }
                })
            }
            None => {
                let (instance, port) = self.ix.birth_wire.get(handle)?;
                let j = self.ix.journal(instance)?;
                let def = self.definition(instance)?;
                let required = Self::spec_node(&def, "requires", port, instance)?;
                // 成员的一个 Required port 接的是 Charter 里的一种来源。链从 Collective 的诞生与
                // Charter 的 Definition 重建，不另记：成员那边的 wire 只有 Handle。
                if let Some(collective) = j.birth().get("collective").and_then(Json::as_str) {
                    let mut out = self.charter_source(collective, j.birth(), port)?;
                    out.push(required);
                    return Some(out);
                }
                let first = match self.self_target(instance, port) {
                    Some(t) if t == "look" => self.self_look_node(instance)?,
                    Some(t) => {
                        Self::spec_node(&def, "provides", t.strip_prefix("port/")?, instance)?
                    }
                    None => Node {
                        spec: ValueId::of_json(
                            &self.ix.journal(instance)?.birth()["binding"]["sources"][port]["spec"],
                        )
                        .ok()?
                        .to_string(),
                        ptr: String::new(),
                        author: None,
                    },
                };
                Some(vec![first, required])
            }
        }
    }

    /// 成员的某个 Required port 在 Charter 里接的那一种来源，作为链的第一节（或前几节）。
    /// 四种来源都能从两份 Definition 推出来：Charter 的 `wire` 指向哪一种，被指的那一份的
    /// Port spec 就是那一节。没有作者：写下它的是 Charter，不是一个 Instance。
    fn charter_source(&self, collective: &str, member: &Json, port: &str) -> Option<Vec<Node>> {
        let charter = self.definition(collective)?;
        let seat = member["seat"].as_str()?;
        let source = charter["wire"].get(seat)?.get(port)?;
        if let Some(outer) = source.get("outer").and_then(Json::as_str) {
            let handle = self.ix.journal(collective)?.birth()["wire"].get(outer)?.as_str()?;
            return self.chain(handle);
        }
        let node = match source.get("self").and_then(Json::as_str) {
            Some(p) => Self::node_at(&charter["seats"][seat]["interface"]["provides"][p])?,
            None => match source.get("seat").and_then(Json::as_str) {
                Some(other) => {
                    let p = source.get("port")?.as_str()?;
                    Self::node_at(&charter["seats"][other]["interface"]["provides"][p])?
                }
                None => {
                    let op = ["look", "notify", "correct"]
                        .into_iter()
                        .find(|op| source.get(op).is_some())?;
                    Node {
                        spec: kernel_spec(op).1.to_string(),
                        ptr: String::new(),
                        author: None,
                    }
                }
            },
        };
        Some(vec![node])
    }

    fn node_at(spec: &Json) -> Option<Node> {
        Some(Node {
            spec: ValueId::of_json(spec).ok()?.to_string(),
            ptr: String::new(),
            author: None,
        })
    }

    /// 一个 Collective 里占着某个 Seat 的成员。成员与 Collective 在同一次事件里诞生，
    /// 所以按诞生的 collective 与 seat 找得到。
    fn member_of(&self, collective: &str, seat: &str) -> Option<String> {
        self.ix
            .journals
            .iter()
            .find(|(_, j)| {
                let b = j.birth();
                b["collective"] == collective && b["seat"] == seat
            })
            .map(|(i, _)| i.clone())
    }

    /// 派生出的链：父的链，接上所经那份 Reference 链上每一节对这个名字写的声明，
    /// 再接上接收方为它写的声明（Required port 的 Port spec）。写的是不限的，不接。
    pub fn derive_chain(
        &self,
        parent: &str,
        via: &str,
        name: &str,
        extra: Option<Node>,
    ) -> Option<Vec<Node>> {
        let mut out = self.chain(parent)?;
        for node in self.chain(via)? {
            if let Some(d) = self.decl(&node, name)? {
                out.push(d);
            }
        }
        out.extend(extra);
        Some(out)
    }

    /// 接收方为附带 `name` 写的、不在所经的链上的声明：实例化时对方的 Required port，
    /// 授予时目标的 Required port。
    pub fn receiver_node(
        &self,
        via: &str,
        value: &Json,
        name: &str,
        born: Option<&str>,
    ) -> Option<Node> {
        match self.provider(via)? {
            Provider::Kernel(k) if k == "instantiate" => {
                let def = self.definition_bundle(value["definition"].as_str()?)?;
                Self::spec_node(&def, "requires", name, born?)
            }
            Provider::Correct(target) if name == "ref" && value["op"] == "grant" => {
                let def = self.definition(&target)?;
                Self::spec_node(&def, "requires", value["port"].as_str()?, &target)
            }
            _ => None,
        }
    }

    /// 一节所在的那份 Port spec 与解开 `$ref` 之后的位置。
    pub fn port_at(&self, node: &Node) -> Option<(Json, String)> {
        let root = self.load(&node.spec)?;
        let mut ptr = node.ptr.clone();
        for _ in 0..8 {
            let here = root.pointer(&ptr)?;
            match here.get("$ref").and_then(Json::as_str) {
                Some(r) if here.get("in").is_none() => {
                    ptr = format!("/$defs/{}", esc(r.strip_prefix("#/$defs/")?))
                }
                _ => return Some((root, ptr)),
            }
        }
        None
    }

    /// 一节对附带 `name` 写的声明：`None` 是不限，不接；外层 `None` 是查不到。
    pub fn decl(&self, node: &Node, name: &str) -> Option<Option<Node>> {
        let (root, ptr) = self.port_at(node)?;
        let attach = root.pointer(&ptr)?.get("attach")?;
        let (leaf, at) = match attach.get("properties").and_then(|p| p.get(name)) {
            Some(leaf) => (leaf, format!("{ptr}/attach/properties/{}", esc(name))),
            None => (
                attach.get("additionalProperties")?,
                format!("{ptr}/attach/additionalProperties"),
            ),
        };
        Some(match leaf {
            Json::Bool(true) => None,
            Json::Bool(false) => return None,
            Json::Object(m) if m.contains_key("$ref") && !m.contains_key("in") => {
                let r = m["$ref"].as_str()?.strip_prefix("#/$defs/")?;
                Some(Node {
                    spec: node.spec.clone(),
                    ptr: format!("/$defs/{}", esc(r)),
                    author: node.author.clone(),
                })
            }
            _ => Some(Node {
                spec: node.spec.clone(),
                ptr: at,
                author: node.author.clone(),
            }),
        })
    }

    pub fn same_node(&self, a: &Node, b: &Node) -> bool {
        let (Some((ra, pa)), Some((rb, pb))) = (self.port_at(a), self.port_at(b)) else {
            return false;
        };
        same_spec(&ra, &pa, &rb, &pb)
    }

    /// `inner` 是 `outer` 的收窄：接收方的声明不比链上的那一节宽。
    pub fn narrow_node(&self, inner: &Node, outer: &Node) -> bool {
        let (Some(a), Some(b)) = (self.port_json(inner), self.port_json(outer)) else {
            return false;
        };
        croon_eval::port::narrows(&a, &b)
    }

    /// 一节的 Port：in、attach、out。
    pub fn port_json(&self, node: &Node) -> Option<Json> {
        let (root, ptr) = self.port_at(node)?;
        root.pointer(&ptr).cloned()
    }

    /// 这一份是否已被撤销：它或它的某个祖先的撤销权被成功地交出过。
    pub fn revoked(&self, handle: &str) -> bool {
        self.ancestors(handle)
            .iter()
            .any(|a| match self.ix.mints.get(a) {
                Some(Mint::Derived { at, name }) => {
                    let (_, send) = what(self.ix.record(at));
                    send["result"]["derived"][name]["revoker"]["handle"]
                        .as_str()
                        .is_some_and(|r| self.ix.ok_sends.contains_key(r))
                }
                _ => false,
            })
    }

    /// 来路：沿派生链往上，第一份由接收方自己交出时派生的；没有，就是所经的那一份。
    pub fn route(&self, handle: &str, receiver: &str) -> String {
        for a in self.ancestors(handle) {
            if let Some(Mint::Derived { at, .. }) = self.ix.mints.get(&a)
                && at.0 == receiver
            {
                return a;
            }
        }
        handle.to_owned()
    }

    /// 向 Notify(target) 登记、还没被撤销的那些：(登记的位置, 派生出的 to)。
    pub fn registrations(&self, target: &str) -> Vec<(Pos, String)> {
        let mut out = Vec::new();
        for (r, poses) in &self.ix.ok_sends {
            if self.provider(r) != Some(Provider::Notify(target.to_owned())) {
                continue;
            }
            for at in poses {
                let (_, send) = what(self.ix.record(at));
                if let Some(h) = send["result"]["derived"]["to"]["handle"].as_str()
                    && !self.revoked(h)
                {
                    out.push((at.clone(), h.to_owned()));
                }
            }
        }
        out.sort();
        out
    }

    // Every record has one fixed delivery set, independent of other Journals' replay order.
    pub(crate) fn record_scope(&self, instance: &str, seq: u64) -> Scope {
        let mut pending = vec![(instance.to_owned(), seq)];
        let mut visited = BTreeSet::new();
        let mut scope = Scope::default();
        while let Some((instance, seq)) = pending.pop() {
            if !visited.insert((instance.clone(), seq)) {
                continue;
            }
            let j = &self.ix.journals[&instance];
            let (kind, body) = what(&j.records[seq as usize]);
            let delivery = |s: &mut Scope, d: &Json| {
                if let Some(id) = d["value"].as_str() {
                    s.values.insert(id.to_owned());
                }
                s.bundles.extend(
                    d["dependencies"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Json::as_str)
                        .map(str::to_owned),
                );
            };
            let incoming = |s: &mut Scope, a: &Json| {
                s.values.extend(
                    a.as_object()
                        .into_iter()
                        .flatten()
                        .filter_map(|(_, v)| ValueId::of_json(&v["spec"]).ok())
                        .map(|id| id.to_string()),
                );
            };
            match kind {
                "birth" => {
                    scope
                        .bundles
                        .insert(body["definition"].as_str().unwrap_or_default().to_owned());
                    delivery(&mut scope, &body["input"]);
                    if let Some(def) = self.definition(&instance) {
                        for group in ["provides", "requires"] {
                            for spec in def["interface"][group]
                                .as_object()
                                .into_iter()
                                .flatten()
                                .map(|(_, v)| v)
                            {
                                if let Ok(id) = ValueId::of_json(spec) {
                                    scope.values.insert(id.to_string());
                                }
                            }
                        }
                    }
                    for source in body["binding"]["sources"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(_, v)| v)
                    {
                        if let Ok(id) = ValueId::of_json(&source["spec"]) {
                            scope.values.insert(id.to_string());
                        }
                    }
                    if body["self"]
                        .as_object()
                        .is_some_and(|m| m.values().any(|v| v == "look"))
                    {
                        scope.values.insert(kernel_spec("look").1.to_string());
                    }
                }
                "send" => {
                    if let Some(id) = body["delivery"]["value"].as_str() {
                        scope.values.insert(id.to_owned());
                    }
                    if body["result"]["accepted"] == true {
                        scope.bundles.extend(
                            body["delivery"]["dependencies"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Json::as_str)
                                .map(str::to_owned),
                        );
                    }
                    for attached in body["attach"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(_, v)| v)
                    {
                        if let Some(spec) =
                            attached["ref"].as_str().and_then(|h| self.source_spec(h))
                        {
                            scope.values.insert(spec);
                        }
                    }
                    let result = &body["result"];
                    if let Ok(id) = ValueId::of_json(&result["outcome"]) {
                        scope.values.insert(id.to_string());
                    }
                    scope.bundles.extend(
                        result["outcome"]["dependencies"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Json::as_str)
                            .map(str::to_owned),
                    );
                    if let Some(Provider::Look(target)) =
                        body["ref"].as_str().and_then(|r| self.provider(r))
                    {
                        if result["outcome"]["ok"] == true {
                            let value = &result["outcome"]["value"];
                            if let Some(records) = value["records"].as_array() {
                                for record in records {
                                    if let Some(seq) = record["seq"].as_u64() {
                                        pending.push((target.clone(), seq));
                                        if let Ok(id) = ValueId::of_json(record) {
                                            scope.values.insert(id.to_string());
                                        }
                                    }
                                }
                            } else if value["outcome"].is_object() {
                                if let Ok(id) = ValueId::of_json(&value["outcome"]) {
                                    scope.values.insert(id.to_string());
                                }
                            }
                        }
                    }
                    incoming(&mut scope, &result["attach"]);
                    for d in result["derived"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(_, v)| v)
                    {
                        if let Ok(id) = ValueId::of_json(&d["revoker"]["spec"]) {
                            scope.values.insert(id.to_string());
                        }
                    }
                }
                "arrival" => {
                    delivery(&mut scope, &body["delivery"]);
                    incoming(&mut scope, &body["attach"]);
                }
                "receive" => {
                    if let Some(n) = body["result"].as_u64() {
                        pending.push((instance.clone(), n));
                    }
                }
                "end" => {
                    if let Ok(id) = ValueId::of_json(body) {
                        scope.values.insert(id.to_string());
                    }
                    scope.bundles.extend(
                        body["dependencies"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Json::as_str)
                            .map(str::to_owned),
                    );
                }
                _ => {}
            }
        }
        scope
    }

    pub(crate) fn scope(
        &self,
        instance: &str,
        consumed: Option<&[String]>,
    ) -> Result<Scope, croon_port::HostError> {
        let j = self
            .ix
            .journal(instance)
            .ok_or_else(|| croon_port::HostError("unknown Instance".into()))?;
        let mut scope = self.record_scope(instance, 0);
        let seqs: Vec<u64> = match consumed {
            None => (1..j.records.len() as u64).collect(),
            Some(keys) => {
                keys.iter()
                    .map(|k| {
                        j.keys.get(k).copied().ok_or_else(|| {
                            croon_port::HostError(format!("unknown consumed key {k}"))
                        })
                    })
                    .collect::<Result<_, _>>()?
            }
        };
        for seq in seqs {
            let added = self.record_scope(instance, seq);
            scope.values.extend(added.values);
            scope.bundles.extend(added.bundles);
        }
        Ok(scope)
    }
}

/// 结束状态里的一条失败。
pub fn failure(kind: &str, message: impl Into<String>, data: Json) -> Json {
    json!({"ok": false, "error": {"kind": kind, "message": message.into(), "data": data}, "dependencies":[]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn birth(seq: u64) -> Json {
        json!({"seq": seq, "at": "t", "by": "outside", "what": {"birth": {"definition": "d", "input": {"value":"i","dependencies":[]}, "wire": {}}}})
    }

    /// 经 `via` 交出，附带 reply = `parent`，派生出 `derived`，撤销权是 `revoker`。
    fn hand(seq: u64, key: &str, via: &str, parent: &str, derived: &str, revoker: &str) -> Json {
        json!({"seq": seq, "at": "t", "by": "self", "what": {"send": {
            "key": key, "to": {"handle": via}, "ref": via, "delivery":{"value":"v","dependencies":[]},
            "attach": {"reply": {"to": {"handle": parent}, "ref": parent}},
            "result": {"outcome": {"ok": true, "value": null}, "attach": {},
                "derived": {"reply": {"handle": derived, "revoker": {"handle": revoker, "spec": "s"}}}}
        }}})
    }

    fn revoke(seq: u64, revoker: &str) -> Json {
        json!({"seq": seq, "at": "t", "by": "self", "what": {"send": {
            "key": "r", "to": {"handle": revoker}, "ref": revoker, "delivery":{"value":"v","dependencies":[]}, "attach": {},
            "result": {"outcome": {"ok": true, "value": null}, "attach": {}, "derived": {}}
        }}})
    }

    /// H 经转交者 F 交给 W，附带回话途径：来路沿派生链报到接收方自己交出的那一份，看不到中间各方的。
    #[test]
    fn a_route_reports_the_first_copy_the_receiver_itself_handed_out() {
        let dir = TempDir::new("route");
        let store = ContentStore::open(&dir.0).unwrap();
        let mut ix = Index::default();
        for i in ["i-h", "i-f", "i-w"] {
            ix.apply(i, birth(0));
        }
        ix.apply("i-h", hand(1, "0", "h-tof", "h-hme", "h-d1", "h-rev1"));
        ix.apply("i-f", hand(1, "0", "h-tow", "h-d1", "h-d2", "h-rev2"));
        let view = View {
            ix: &ix,
            store: &store,
        };
        assert_eq!(view.ancestors("h-d2"), ["h-d2", "h-d1", "h-hme"]);
        assert_eq!(
            view.route("h-d2", "i-h"),
            "h-d1",
            "H 认出的是自己交出的那一份"
        );
        assert_eq!(view.route("h-d2", "i-f"), "h-d2");
        assert_eq!(view.route("h-d2", "i-w"), "h-d2", "W 只看到所经的那一份");
        assert_eq!(view.route("h-hme", "i-w"), "h-hme");

        assert!(!view.revoked("h-d2") && !view.revoked("h-d1"));
        ix.apply("i-h", revoke(2, "h-rev1"));
        let view = View {
            ix: &ix,
            store: &store,
        };
        assert!(
            view.revoked("h-d1") && view.revoked("h-d2"),
            "撤销沿派生链递归"
        );
        assert!(!view.revoked("h-hme"));
    }

    /// Charter 接上的那一份：链从 Collective 的诞生与 Charter 的 Definition 重建。
    /// 成员的 wire 里只有 Handle，来源不记在成员那边。
    #[test]
    fn a_member_wire_chain_is_rebuilt_from_the_charter() {
        let dir = TempDir::new("charter");
        let store = ContentStore::open(&dir.0).unwrap();
        let put = |v: Json| -> String {
            let content = store.put_json(&v).unwrap().to_string();
            let b = json!({"content": content, "dependencies": []});
            store.put_json(&b).unwrap().to_string()
        };
        let run = json!({"description": "跑", "in": {"type": "null"}, "attach": {"additionalProperties": false}});
        let back = json!({"description": "回", "in": {"type": "null"}, "attach": {"additionalProperties": false}});
        let sees = json!({"description": "看", "in": {}, "attach": {"additionalProperties": false}});
        let charter_def = json!({
            "interface": {"description": "集体", "signature": {"in": {}, "out": {}}, "provides": {}, "requires": {"run": sees}},
            "seats": {
                "gate": {"interface": {"description": "门", "signature": {"in": {}, "out": {}}, "provides": {"pass": back}, "requires": {"up": run}}},
                "audit": {"interface": {"description": "审", "signature": {"in": {}, "out": {}}, "provides": {}, "requires": {"see": sees}}}
            },
            "wire": {"gate": {"up": {"outer": "run"}}, "audit": {"see": {"seat": "gate", "port": "pass"}}},
            "export": {"signature": "gate", "provides": {}}
        });
        store_specs(&store, &charter_def);
        let charter = put(charter_def);
        let member_def = json!({
            "language": "fake@1",
            "interface": {"description": "门", "signature": {"in": {}, "out": {}}, "provides": {"pass": back}, "requires": {"up": run}},
            "body": "gate"
        });
        store_specs(&store, &member_def);
        let member = put(member_def);
        let auditor_def = json!({
            "language": "fake@1",
            "interface": {"description": "审", "signature": {"in": {}, "out": {}}, "provides": {}, "requires": {"see": sees}},
            "body": "audit"
        });
        store_specs(&store, &auditor_def);
        let auditor = put(auditor_def);

        let mut ix = Index::default();
        ix.apply(
            "i-c",
            json!({"seq": 0, "at": "t", "by": "i-root", "event": "e-born", "what": {"birth": {
                "definition": charter, "input": {"value": "i", "dependencies": []},
                "wire": {"run": "h-crun"}, "staffing": {}
            }}}),
        );
        ix.apply(
            "i-gate",
            json!({"seq": 0, "at": "t", "by": "i-zero", "event": "e-born", "what": {"birth": {
                "definition": member, "input": {"value": "i", "dependencies": []},
                "wire": {"up": "h-gateup"}, "collective": "i-c", "seat": "gate"
            }}}),
        );
        ix.apply(
            "i-audit",
            json!({"seq": 0, "at": "t", "by": "i-zero", "event": "e-born", "what": {"birth": {
                "definition": auditor, "input": {"value": "i", "dependencies": []},
                "wire": {"see": "h-ausee"}, "collective": "i-c", "seat": "audit"
            }}}),
        );
        let view = View {
            ix: &ix,
            store: &store,
        };

        // seat → seat：第一节是被指 Seat 的 Provided port，第二节是成员自己写的 Required port。
        let chain = view.chain("h-ausee").expect("成员接上的那一份有链");
        assert_eq!(chain.len(), 2, "来源一节加自己一节");
        assert_eq!(node_at_json(&view, &chain[0]), back);
        assert_eq!(node_at_json(&view, &chain[1]), sees);
        assert_eq!(
            view.provider("h-ausee"),
            Some(Provider::Provided {
                instance: "i-gate".to_owned(),
                port: "pass".to_owned()
            })
        );
    }

    /// 链上的每一节都按「一份 Port spec 是一个 Value」解开，所以 Port spec 本身要进内容库。
    fn store_specs(store: &ContentStore, def: &Json) {
        let sides = [&def["interface"], &def["seats"]];
        for side in sides {
            let ifaces: Vec<&Json> = if side.get("interface").is_some() {
                vec![&side["interface"]]
            } else {
                side.as_object()
                    .into_iter()
                    .flatten()
                    .map(|(_, seat)| &seat["interface"])
                    .collect()
            };
            for iface in ifaces {
                for port in ["provides", "requires"] {
                    for spec in iface[port].as_object().into_iter().flatten().map(|(_, s)| s) {
                        store.put_json(spec).unwrap();
                    }
                }
            }
        }
    }

    fn node_at_json(view: &View, node: &Node) -> Json {
        view.port_json(node).expect("这一节能解开")
    }

    #[test]
    fn same_spec_ignores_descriptions_but_not_fields_named_description() {
        let a = json!({"description": "x", "in": {"type": "object", "properties": {"description": {"type": "string"}}}, "attach": {"additionalProperties": false}});
        let mut b = a.clone();
        b["description"] = json!("y");
        assert!(same_spec(&a, "", &b, ""));
        b["in"]["properties"] = json!({});
        assert!(!same_spec(&a, "", &b, ""));
    }
}
