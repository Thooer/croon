//! Kernel：一次事务分三步，决定、提交、更新索引。
//! 参考：docs/Kernel/Journal.md（实现）。
//!
//! 只有决定读索引、做判断，判断都写进记录；提交在记录库的一个事务里查代际号、写下记录；
//! 更新索引与重建走同一个 `Index::apply`。第一版同一时刻只决定、提交一个事务。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use serde_json::{Value as Json, json};

use crate::host::{
    Call, Channel, CheckError, Config, Execution, Fault, HostDone, Port, Read, Session,
};
use crate::ids::{IdGen, IdKind};
use crate::index::{
    Index, Mint, Node, Provider, View, failure, kernel_ports, kernel_spec, same_spec, what,
};
use crate::journal::{Journal, JournalError, NewRecord};
use croon_eval::schema::{json_eq, validate};
use croon_eval::value::{ValueError, ValueId};
use croon_port::{Bundle, Content, ContentStore, Scope, StoreError};

mod content;
mod execution;
mod read;
#[cfg(test)]
mod tests;
use read::Reader;

#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error(transparent)]
    Journal(#[from] JournalError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error("Kernel 已退出，要重开")]
    Crashed,
    #[error("这次会话已过期：{0} 的执行权在新的会话手里")]
    Stale(String),
    #[error("{0} 已被停下：{1}")]
    Stopped(String, String),
    #[error("{0}")]
    Refused(String),
    #[error("宿主故障：{0}")]
    Host(String),
}

/// 一次会话：求值器在系统外的 Instance 的客户端连上时得到。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conn {
    pub instance: String,
    pub generation: u64,
}

/// 报出了、还没有记录的 Atom。暂停时不执行：没有世界效果的丢弃，解除暂停后重新执行。
struct Flight {
    atom: Json,
    generation: u64,
    accepted: Option<AcceptedSend>,
}

impl Flight {
    fn world_effect(&self) -> bool {
        self.accepted
            .as_ref()
            .is_some_and(|a| a.channel == Some(Channel::Isolated))
    }
}

struct AcceptedSend {
    reference: Option<String>,
    refs: BTreeMap<String, Option<String>>,
    provider: Option<Provider>,
    chain: Vec<Node>,
    channel: Option<Channel>,
    execution: Option<Box<dyn Execution>>,
    completed: Option<HostDone>,
    content: Option<Rc<Content>>,
    port: Option<Rc<dyn Port>>,
    attempt: String,
    consumed: bool,
    atom: Json,
    started: bool,
}

struct Live {
    generation: u64,
    connection: Option<u64>,
    session: Option<Box<dyn Session>>,
    external: bool,
    inflight: BTreeMap<String, Flight>,
    outbox: BTreeMap<String, Json>,
}

impl Live {
    fn new(external: bool) -> Live {
        Live {
            generation: 0,
            connection: None,
            session: None,
            external,
            inflight: BTreeMap::new(),
            outbox: BTreeMap::new(),
        }
    }
}

enum Write {
    Create(String, NewRecord),
    Step(String, u64, NewRecord),
    Caused(String, NewRecord),
}

enum Effect {
    Spawned(String),
    Ended(String),
}

#[derive(Default)]
struct Plan {
    writes: Vec<Write>,
    effects: Vec<Effect>,
}

/// 一个 Kernel 的 Port 或别人的 Port 给出的结果，加上它在别处造成的记录。
struct Outcome {
    outcome: Json,
    attach: Json,
    derived: Json,
    caused: Vec<(String, NewRecord)>,
    creates: Vec<(String, NewRecord)>,
    effects: Vec<Effect>,
    accepted: bool,
}

impl Outcome {
    fn of(outcome: Json) -> Outcome {
        Outcome {
            outcome,
            attach: json!({}),
            derived: json!({}),
            caused: Vec::new(),
            creates: Vec::new(),
            effects: Vec::new(),
            accepted: true,
        }
    }
    fn ok(value: Json) -> Outcome {
        Outcome::of(json!({"ok": true, "value": value, "dependencies":[]}))
    }
    fn fail(kind: &str, message: impl Into<String>) -> Outcome {
        Outcome::of(failure(kind, message, Json::Null))
    }
    fn rejected(kind: &str, message: impl Into<String>) -> Outcome {
        let mut outcome = Self::fail(kind, message);
        outcome.accepted = false;
        outcome
    }
}

pub struct Kernel {
    dir: PathBuf,
    journal: Journal,
    store: Rc<ContentStore>,
    ids: IdGen,
    index: Index,
    config: Config,
    live: BTreeMap<String, Live>,
    stopped: BTreeMap<String, String>,
    stopped_world_effect: BTreeMap<String, bool>,
    dead: bool,
}

fn atom_schema() -> &'static Json {
    static S: OnceLock<Json> = OnceLock::new();
    S.get_or_init(|| {
        let out = crate::blocks::json("definition.eval")["out"].clone();
        json!({"$ref": "#/$defs/atom", "$defs": out["$defs"]})
    })
}

fn eval_out() -> &'static Json {
    static S: OnceLock<Json> = OnceLock::new();
    S.get_or_init(|| crate::blocks::json("definition.eval")["out"].clone())
}

fn outcome_schema() -> &'static Json {
    static S: OnceLock<Json> = OnceLock::new();
    S.get_or_init(|| crate::blocks::json("definition.outcome"))
}

impl Kernel {
    /// 打开一个系统：重开记录库与内容库，从全部记录重建索引。运行中的 Instance 在下一次
    /// `run` 时各自取得执行权、重开会话；求值器在系统外的，等客户端连上。
    pub fn open(dir: impl AsRef<Path>, config: Config) -> Result<Kernel, KernelError> {
        let dir = dir.as_ref().to_owned();
        std::fs::create_dir_all(&dir)
            .map_err(|e| KernelError::Refused(format!("{}：{e}", dir.display())))?;
        let journal = Journal::open(dir.join("journal.db"))?;
        let store = Rc::new(ContentStore::open(dir.join("values"))?);
        let ids = IdGen::new(&system_key(&dir)?);
        for k in kernel_ports() {
            store.put_json(kernel_spec(k).0)?;
        }
        store.put_json(&json!({}))?;
        let index = Index::rebuild(journal.all()?);
        let mut stopped = BTreeMap::new();
        let mut stopped_world_effect = BTreeMap::new();
        for (instance, reason, world_effect) in journal.stopped()? {
            stopped.insert(instance.clone(), reason);
            stopped_world_effect.insert(instance, world_effect);
        }
        let mut kernel = Kernel {
            dir,
            journal,
            store,
            ids,
            index,
            config,
            live: BTreeMap::new(),
            stopped,
            stopped_world_effect,
            dead: false,
        };
        let running: Vec<String> = kernel
            .index
            .journals
            .iter()
            .filter(|(i, j)| j.ended.is_none() && !kernel.stopped.contains_key(*i))
            .map(|(i, _)| i.clone())
            .collect();
        for i in running {
            let Some(def) = kernel.view().definition(&i) else {
                continue;
            };
            // Collective 没有 body，也没有 Language，不进要开求值会话的表，与实例化时一样。
            // 见 docs/Kernel/Instance.md 的「成员的身份」。
            if def.get("seats").is_some() {
                continue;
            }
            let external = def.get("evaluator").is_some();
            kernel.live.insert(i, Live::new(external));
        }
        if kernel.can_recover_attempts()? {
            kernel.recover_attempts()?;
        }
        Ok(kernel)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn store(&self) -> &Rc<ContentStore> {
        &self.store
    }

    pub fn stopped(&self) -> &BTreeMap<String, String> {
        &self.stopped
    }

    /// 报出了、还没有记录的 Atom 的键：暂停时丢弃的生成就在这里。
    pub fn inflight(&self, instance: &str) -> Vec<String> {
        self.live
            .get(instance)
            .map(|l| l.inflight.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn view(&self) -> View<'_> {
        View {
            ix: &self.index,
            store: &self.store,
        }
    }

    fn alive(&self) -> Result<(), KernelError> {
        if self.dead {
            Err(KernelError::Crashed)
        } else {
            Ok(())
        }
    }

    fn handle(&self, parts: &[&str]) -> String {
        self.ids.derive(IdKind::Handle, parts)
    }

    fn event(&self, parts: &[&str]) -> String {
        self.ids.derive(IdKind::Event, parts)
    }

    fn record(&mut self, by: &str, event: Option<&str>, what: Json) -> NewRecord {
        NewRecord {
            at: (self.config.clock)(),
            by: by.to_owned(),
            event: event.map(str::to_owned),
            what,
        }
    }

    fn put_specs(&self, definition: &Json) -> Result<(), KernelError> {
        for side in ["provides", "requires"] {
            for spec in definition["interface"][side]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(_, s)| s)
            {
                self.store.put_json(spec)?;
            }
        }
        Ok(())
    }

    /// Trusted root import establishes content, not an Instance's reading qualification.
    pub fn import(&self, value: &Json) -> Result<ValueId, KernelError> {
        Ok(self.store.put_json(value)?)
    }
    pub fn import_bytes(&self, bytes: &[u8]) -> Result<ValueId, KernelError> {
        Ok(self.store.put_bytes(bytes)?)
    }
    pub fn import_bundle(
        &self,
        content: &str,
        dependencies: &[String],
    ) -> Result<String, KernelError> {
        let mut scope = Scope::default();
        scope.values.insert(content.to_owned());
        scope.bundles.extend(dependencies.iter().cloned());
        let access =
            Content::new(self.store.clone(), scope, self.config.budgets).map_err(host_error)?;
        if access.bytes(content).map_err(host_error)?.is_none() {
            return Err(KernelError::Refused("missing content".into()));
        }
        access.pack(content, dependencies).map_err(host_error)
    }

    // ---- 提交 ----

    fn fault(&mut self, point: Fault) -> bool {
        let hit = self.config.fault.as_mut().is_some_and(|f| f(&point));
        if hit {
            self.dead = true;
        }
        hit
    }

    fn commit(&mut self, plan: Plan) -> Result<(), KernelError> {
        self.alive()?;
        let keys: Vec<(String, Option<String>)> = plan
            .writes
            .iter()
            .map(|w| {
                let (i, r) = match w {
                    Write::Create(i, r) | Write::Step(i, _, r) | Write::Caused(i, r) => (i, r),
                };
                let key = ["send", "receive"]
                    .iter()
                    .find_map(|k| r.what.get(k)?.get("key")?.as_str())
                    .map(str::to_owned);
                (i.clone(), key)
            })
            .collect();
        if self.fault(Fault::BeforeCommit {
            records: keys.clone(),
        }) {
            return Err(KernelError::Crashed);
        }
        for write in &plan.writes {
            let record = match write {
                Write::Create(_, r) | Write::Step(_, _, r) | Write::Caused(_, r) => r,
            };
            self.persist_record_values(&record.what)?;
        }
        let written: Vec<(String, u64)> = self.journal.transaction(|tx| {
            let mut out = Vec::new();
            for w in plan.writes {
                match w {
                    Write::Create(i, r) => {
                        tx.create(&i, r)?;
                        out.push((i, 0));
                    }
                    Write::Step(i, g, r) => {
                        let seq = tx.append_step(&i, g, r)?;
                        out.push((i, seq));
                    }
                    Write::Caused(i, r) => {
                        let seq = tx.append_caused(&i, r)?;
                        out.push((i, seq));
                    }
                }
            }
            Ok(out)
        })?;
        if self.fault(Fault::AfterCommit { records: keys }) {
            return Err(KernelError::Crashed);
        }
        for (i, seq) in written {
            let record = self
                .journal
                .read(&i, seq, 1)?
                .pop()
                .expect("刚写下的记录读得到");
            self.index.apply(&i, record);
        }
        for e in plan.effects {
            match e {
                Effect::Spawned(i) => {
                    let external = self
                        .view()
                        .definition(&i)
                        .is_some_and(|d| d.get("evaluator").is_some());
                    self.live.insert(i, Live::new(external));
                }
                Effect::Ended(i) => {
                    self.live.remove(&i);
                }
            }
        }
        Ok(())
    }

    fn persist_record_values(&self, what: &Json) -> Result<(), KernelError> {
        for kind in ["send", "arrival"] {
            if let Some(body) = what.get(kind) {
                for incoming in body["attach"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(_, v)| v)
                {
                    if incoming["spec"].is_object() {
                        self.store.put_json(&incoming["spec"])?;
                    }
                }
                if kind == "send" {
                    self.store.put_json(&body["result"]["outcome"])?;
                    for incoming in body["result"]["attach"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(_, v)| v)
                    {
                        self.store.put_json(&incoming["spec"])?;
                    }
                    for d in body["result"]["derived"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(_, v)| v)
                    {
                        self.store.put_json(&d["revoker"]["spec"])?;
                    }
                }
            }
        }
        Ok(())
    }

    fn stop(&mut self, instance: &str, reason: String) -> Result<(), KernelError> {
        tracing::warn!(instance, %reason, "停下");
        let world_effect = self
            .live
            .get(instance)
            .is_some_and(|live| live.inflight.values().any(Flight::world_effect))
            || self
                .journal
                .attempts()?
                .iter()
                .any(|(_, i, _, _, _)| i == instance);
        self.journal.stop(instance, &reason, world_effect)?;
        self.stopped.insert(instance.to_owned(), reason);
        self.stopped_world_effect
            .insert(instance.to_owned(), world_effect);
        if let Some(live) = self.live.get_mut(instance) {
            live.session = None;
        }
        Ok(())
    }

    /// 修复宿主实现后重开被停下的 Instance，按原 Definition 恢复求值位置。
    pub fn reopen(&mut self, instance: &str) -> Result<(), KernelError> {
        self.alive()?;
        let Some(reason) = self.stopped.get(instance) else {
            return Err(KernelError::Refused(format!("{instance} 没有宿主停止标记")));
        };
        if self
            .journal
            .attempts()?
            .iter()
            .any(|(_, i, _, _, _)| i == instance)
            || self
                .stopped_world_effect
                .get(instance)
                .copied()
                .unwrap_or(false)
            || self
                .live
                .get(instance)
                .is_some_and(|live| live.inflight.values().any(Flight::world_effect))
        {
            return Err(KernelError::Stopped(
                instance.to_owned(),
                format!("{reason}；仍有未确认的世界效果，不能安全重开"),
            ));
        }
        let Some(journal) = self.index.journal(instance) else {
            return Err(KernelError::Refused(format!("{instance} 不存在")));
        };
        if journal.ended.is_some() {
            return Err(KernelError::Refused(format!("{instance} 已结束")));
        }
        self.journal.resume_stopped(instance)?;
        self.stopped.remove(instance);
        self.stopped_world_effect.remove(instance);
        let external = self
            .view()
            .definition(instance)
            .is_some_and(|d| d.get("evaluator").is_some());
        self.live
            .entry(instance.to_owned())
            .or_insert_with(|| Live::new(external));
        if let Some(live) = self.live.get_mut(instance) {
            live.session = None;
            live.connection = None;
            for (_, flight) in std::mem::take(&mut live.inflight) {
                discard(flight);
            }
            live.generation = 0;
        }
        Ok(())
    }

    // ---- root entry ----

    /// Root creation always names its complete Definition and all source bindings explicitly.
    pub fn create_root(
        &mut self,
        definition_bundle: &str,
        input: &Json,
        self_map: &Json,
        binding: &Json,
    ) -> Result<String, KernelError> {
        self.alive()?;
        if self.index.root().is_some() {
            return Err(KernelError::Refused("a root already exists".into()));
        }
        validate(&crate::blocks::json("value.delivery"), input)
            .map_err(|e| KernelError::Refused(format!("input: {e:?}")))?;
        if !canonical_dependencies(&input["dependencies"]) {
            return Err(KernelError::Refused(
                "input dependencies must be sorted unique Bundle IDs".into(),
            ));
        }
        validate(&crate::blocks::json("reference.binding"), binding)
            .map_err(|e| KernelError::Refused(format!("binding: {e:?}")))?;
        let mut scope = Scope::default();
        scope.bundles.insert(definition_bundle.to_owned());
        scope.bundles.extend(strings(&input["dependencies"]));
        Content::new(self.store.clone(), scope, self.config.budgets).map_err(host_error)?;
        let definition = self
            .view()
            .definition_bundle(definition_bundle)
            .ok_or_else(|| KernelError::Refused("missing Definition".into()))?;
        croon_eval::definition::check_shape(&definition)
            .map_err(|e| KernelError::Refused(format!("Definition: {e:?}")))?;
        if definition["evaluator"] != "external" {
            return Err(KernelError::Refused(
                "root evaluator must be external".into(),
            ));
        }
        let iface = &definition["interface"];
        validate(&iface["signature"]["in"], &input["value"])
            .map_err(|e| KernelError::Refused(format!("Signature: {e:?}")))?;
        let self_map = self_map
            .as_object()
            .ok_or_else(|| KernelError::Refused("self must be an object".into()))?;
        let wanted: BTreeSet<_> = iface["requires"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|p| !self_map.contains_key(*p))
            .collect();
        let given: BTreeSet<_> = binding["sources"].as_object().unwrap().keys().collect();
        if wanted != given || self_map.keys().any(|p| iface["requires"].get(p).is_none()) {
            return Err(KernelError::Refused(
                "binding must cover exactly non-self Required ports".into(),
            ));
        }
        for (port, spec) in iface["requires"].as_object().unwrap() {
            let actual = if let Some(t) = self_map.get(port) {
                let t = t
                    .as_str()
                    .ok_or_else(|| KernelError::Refused("self target must be a string".into()))?;
                if t == "look" {
                    kernel_spec("look").0.clone()
                } else {
                    t.strip_prefix("port/")
                        .and_then(|p| iface["provides"].get(p))
                        .cloned()
                        .ok_or_else(|| KernelError::Refused("invalid self target".into()))?
                }
            } else {
                let source = &binding["sources"][port];
                let actual = &source["spec"];
                let probe = json!({"evaluator":"external","interface":{"description":"","signature":{"in":{},"out":{}},"provides":{},"requires":{"source":actual}}});
                croon_eval::definition::check_shape(&probe)
                    .map_err(|e| KernelError::Refused(format!("source spec: {e:?}")))?;
                if let Some(op) = source["kernel"].as_str() {
                    if !same_spec(actual, "", kernel_spec(op).0, "") {
                        return Err(KernelError::Refused("Kernel binding has wrong spec".into()));
                    }
                }
                self.store.put_json(actual)?;
                actual.clone()
            };
            if !same_spec(&actual, "", spec, "") {
                return Err(KernelError::Refused(format!(
                    "{port}: source and Required port differ"
                )));
            }
        }
        self.put_specs(&definition)?;
        let input_id = self.store.put_json(&input["value"])?;
        let root = self.ids.derive(IdKind::Instance, &["root"]);
        let wire: serde_json::Map<String, Json> = iface["requires"]
            .as_object()
            .unwrap()
            .keys()
            .map(|p| (p.clone(), json!(self.handle(&[&root, "wire", p]))))
            .collect();
        let birth = json!({"definition":definition_bundle,"input":{"value":input_id.as_str(),"dependencies":input["dependencies"]},"wire":wire,"self":self_map,"binding":binding});
        let record = self.record("outside", None, json!({"birth":birth}));
        self.commit(Plan {
            writes: vec![Write::Create(root.clone(), record)],
            effects: vec![Effect::Spawned(root.clone())],
        })?;
        Ok(root)
    }

    /// 按持久来源装实现；已接受的执行持有自己的对象，不受换装影响。
    pub fn bind_host(
        &mut self,
        source: &str,
        port: std::rc::Rc<dyn Port>,
    ) -> Result<(), KernelError> {
        if self.view().provider(source) != Some(Provider::Host(source.to_owned())) {
            return Err(KernelError::Refused("宿主装配要指向根的源 Handle".into()));
        }
        let root = self.index.birth_wire.get(source).unwrap();
        let identity = &self.index.journals[&root.0].birth()["binding"]["sources"][&root.1]["host"];
        if identity.as_str() != Some(port.id()) {
            return Err(KernelError::Refused(
                "host recovery identity mismatch".into(),
            ));
        }
        self.config.ports.insert(source.to_owned(), port);
        if self.can_recover_attempts()? {
            for (_, instance, _, _, _) in self.journal.attempts()? {
                if self
                    .stopped
                    .get(&instance)
                    .is_some_and(|reason| reason.starts_with("original recovery provider"))
                {
                    self.journal.resume_stopped(&instance)?;
                    self.stopped.remove(&instance);
                    self.stopped_world_effect.remove(&instance);
                }
            }
            self.recover_attempts()?;
        }
        Ok(())
    }

    fn can_recover_attempts(&self) -> Result<bool, KernelError> {
        Ok(self
            .journal
            .attempts()?
            .iter()
            .all(|(_, _, _, _, snapshot)| {
                snapshot["provider"]
                    .as_str()
                    .is_none_or(|id| self.config.ports.values().any(|port| port.id() == id))
            }))
    }

    /// 客户端连上：取得执行权，代际号加一，旧会话的交出从此被拒。
    pub fn connect(&mut self, instance: &str) -> Result<Conn, KernelError> {
        self.alive()?;
        if let Some(reason) = self.stopped.get(instance) {
            return Err(KernelError::Stopped(instance.to_owned(), reason.clone()));
        }
        let external = self.live.get(instance).is_some_and(|l| l.external);
        if !external {
            return Err(KernelError::Refused(format!(
                "{instance} 不是运行中的、求值器在系统外的 Instance"
            )));
        }
        if self.live[instance]
            .inflight
            .values()
            .any(Flight::world_effect)
        {
            return Err(KernelError::Refused(
                "仍有已接受的世界效果，收齐后才能换代际".into(),
            ));
        }
        let generation = self.journal.transaction(|tx| tx.acquire(instance))?;
        let live = self.live.get_mut(instance).expect("上面查过");
        live.generation = generation;
        live.connection = Some(generation);
        for (_, f) in std::mem::take(&mut live.inflight) {
            discard(f);
        }
        Ok(Conn {
            instance: instance.to_owned(),
            generation,
        })
    }

    /// 求值器在系统外的一方报出一个 Atom。有结果就给出；还没有（如等着的取），返回 None，
    /// 之后用同一个键重报。
    pub fn submit(&mut self, conn: &Conn, atom: &Json) -> Result<Option<Json>, KernelError> {
        self.alive()?;
        let i = conn.instance.as_str();
        if let Some(reason) = self.stopped.get(i) {
            let key = atom["key"].as_str().unwrap_or_default();
            let collecting = self
                .live
                .get(i)
                .and_then(|l| l.inflight.get(key))
                .is_some_and(|f| f.world_effect() && json_eq(&f.atom, atom));
            if !collecting {
                return Err(KernelError::Stopped(i.to_owned(), reason.clone()));
            }
        }
        if self
            .live
            .get(i)
            .is_none_or(|l| l.connection != Some(conn.generation))
        {
            return Err(KernelError::Stale(i.to_owned()));
        }
        if let Err(v) = validate(atom_schema(), atom) {
            return Err(KernelError::Refused(format!(
                "报出的不合 definition.eval 的 atom：{v:?}"
            )));
        }
        let offered = match self.offer(i, atom) {
            Err(error @ KernelError::Host(_)) => {
                if let Some(live) = self.live.get_mut(i) {
                    let key = atom["key"].as_str().unwrap_or_default();
                    if live.inflight.get(key).is_some_and(|f| {
                        !f.world_effect()
                            && f.accepted.as_ref().is_none_or(|a| a.execution.is_none())
                    }) {
                        live.inflight.remove(key);
                    }
                }
                if self
                    .live
                    .get(i)
                    .is_some_and(|l| l.inflight.values().any(Flight::world_effect))
                {
                    self.stop(i, format!("外部 Entry 的已接受世界效果收取故障：{error}"))?;
                }
                return Err(error);
            }
            result => result?,
        };
        match offered {
            Offered::Result(r) => Ok(Some(r)),
            Offered::Pending => Ok(None),
            Offered::Diverged(why) => {
                // 会话失效不改变已接受执行持有的代际；收齐后客户端才能重新连上。
                if let Some(l) = self.live.get_mut(i) {
                    l.connection = None;
                }
                Err(KernelError::Refused(format!("重放分歧：{why}")))
            }
        }
    }

    // ---- 会话 ----

    /// 驱动求值器在宿主的 Instance，直到没有进展。
    pub fn run(&mut self) -> Result<bool, KernelError> {
        let mut any = false;
        loop {
            self.alive()?;
            let mut progress = false;
            let ids: Vec<String> = self.live.keys().cloned().collect();
            for i in ids {
                progress |= self.poll(&i)?;
            }
            any |= progress;
            if !progress {
                return Ok(any);
            }
        }
    }

    fn results_of(&self, instance: &str) -> Result<BTreeMap<String, Json>, KernelError> {
        let j = &self.index.journals[instance];
        j.keys
            .iter()
            .map(|(k, seq)| {
                self.result_of(instance, &j.records[*seq as usize])
                    .map(|r| (k.clone(), r))
            })
            .collect()
    }

    /// 一条交出或取的记录给 Language 的结果，形状见 definition.eval 的 results。
    fn result_of(&self, instance: &str, record: &Json) -> Result<Json, KernelError> {
        let (kind, body) = what(record);
        if kind == "send" {
            return Ok(body["result"].clone());
        }
        let arrival = if let Some(n) = body["result"].as_u64() {
            let (_, a) = what(&self.index.journals[instance].records[n as usize]);
            let id = ValueId::parse(a["delivery"]["value"].as_str().unwrap_or_default())
                .ok_or_else(|| KernelError::Host("invalid arrival content ID".into()))?;
            let value = self
                .store
                .get_json(&id)?
                .ok_or_else(|| KernelError::Host("missing arrival content".into()))?;
            json!({"port":a["port"],"sender":a["sender"],"via":a["via"],"delivery":{"value":value,"dependencies":a["delivery"]["dependencies"]},"attach":a["attach"]})
        } else {
            Json::Null
        };
        Ok(json!({"arrival":arrival}))
    }

    fn poll(&mut self, i: &str) -> Result<bool, KernelError> {
        match self.poll_live(i) {
            Err(error @ KernelError::Host(_)) => {
                if self.stopped.contains_key(i) {
                    return Ok(false);
                }
                self.stop(i, format!("在途 Atom 宿主故障：{error}"))?;
                Ok(true)
            }
            result => result,
        }
    }

    fn poll_live(&mut self, i: &str) -> Result<bool, KernelError> {
        if self.stopped.contains_key(i) {
            return self.retry(i);
        }
        let Some(live) = self.live.get(i) else {
            return Ok(false);
        };
        if live.external {
            return self.retry(i);
        }
        if live.session.is_none() && live.inflight.values().any(Flight::world_effect) {
            return self.retry(i);
        }
        let paused = self.index.journals[i].paused;
        let mut progress = false;
        if paused {
            return self.retry(i);
        }
        if live.session.is_none() {
            let generation = if self.live[i].generation == 0 {
                self.journal.transaction(|tx| tx.acquire(i))?
            } else {
                self.live[i].generation
            };
            self.live.get_mut(i).expect("上面查过").generation = generation;
            let j = &self.index.journals[i];
            let birth = j.birth();
            let def = self.view().definition(i).unwrap_or(Json::Null);
            let input_id = ValueId::parse(birth["input"]["value"].as_str().unwrap_or_default())
                .ok_or_else(|| KernelError::Host("invalid birth input".into()))?;
            let input = self
                .store
                .get_json(&input_id)?
                .ok_or_else(|| KernelError::Host("missing birth input".into()))?;
            let lang_id = def["language"].as_str().unwrap_or_default().to_owned();
            let eval_birth = json!({"definition": def, "bundle":birth["definition"], "input":{"value":input,"dependencies":birth["input"]["dependencies"]}});
            let results = self.results_of(i)?;
            let Some(n) = self.config.languages.iter().position(|l| l.id() == lang_id) else {
                self.stop(i, format!("Language {lang_id} 没有装"))?;
                return Ok(true);
            };
            let reader = Reader::new(self, i, None);
            let session = match self.config.languages[n].open(i, &eval_birth, &results, &reader) {
                Ok(session) => session,
                Err(error) => {
                    self.stop(i, format!("Language {lang_id} 打开会话失败：{error}"))?;
                    return Ok(true);
                }
            };
            let live = self.live.get_mut(i).expect("上面查过");
            live.generation = generation;
            live.session = Some(session);
            progress = true;
        }
        progress |= self.retry(i)?;
        if paused || self.stopped.contains_key(i) || !self.live.contains_key(i) {
            return Ok(progress);
        }
        let live = self.live.get_mut(i).expect("上面查过");
        let delta = std::mem::take(&mut live.outbox);
        progress |= !delta.is_empty();
        let mut session = live.session.take().expect("上面开了会话");
        let reader = Reader::new(self, i, None);
        let out = match session.resume(&delta, &reader) {
            Ok(out) => out,
            Err(error) => {
                if let Some(l) = self.live.get_mut(i) {
                    for (key, result) in delta {
                        l.outbox.insert(key, result);
                    }
                }
                self.stop(i, format!("Language 会话故障：{error}"))?;
                return Ok(true);
            }
        };
        if let Some(l) = self.live.get_mut(i) {
            l.session = Some(session);
        }
        if let Err(v) = validate(eval_out(), &out) {
            self.stop(
                i,
                format!("Language 报出的不合 definition.eval 的 out：{v:?}"),
            )?;
            return Ok(true);
        }
        if let Some(atoms) = out.get("atoms").and_then(Json::as_array) {
            for atom in atoms {
                let key = atom["key"].as_str().unwrap_or_default().to_owned();
                let known = self
                    .live
                    .get(i)
                    .is_some_and(|l| l.inflight.contains_key(&key));
                match self.offer(i, atom) {
                    Ok(Offered::Result(r)) => {
                        if let Some(l) = self.live.get_mut(i) {
                            l.outbox.insert(key, r);
                        }
                        progress = true;
                    }
                    Ok(Offered::Pending) => progress |= !known,
                    Ok(Offered::Diverged(why)) => {
                        self.stop(i, format!("重放分歧：{why}"))?;
                        return Ok(true);
                    }
                    Err(error @ KernelError::Host(_)) => {
                        self.stop(i, format!("Language 报出的 Atom 故障：{error}"))?;
                        return Ok(true);
                    }
                    Err(error) => return Err(error),
                }
                if !self.live.contains_key(i) {
                    return Ok(true);
                }
            }
        } else if let Some(done) = out.get("done") {
            if self.live.get(i).is_some_and(|l| !l.inflight.is_empty()) {
                self.stop(i, "报出的 Atom 没有结果时给出了 done".into())?;
                return Ok(true);
            }
            let plan = self.decide_end(i, done)?;
            self.commit(plan)?;
            progress = true;
        }
        Ok(progress)
    }

    /// 在途的 Atom：无世界效果的重试决定，已接受世界效果仅从固定 Execution 收取。
    fn retry(&mut self, i: &str) -> Result<bool, KernelError> {
        let keys: Vec<String> = self
            .live
            .get(i)
            .map(|l| l.inflight.keys().cloned().collect())
            .unwrap_or_default();
        let mut progress = false;
        for key in keys {
            if let Some(r) = self.attempt(i, &key)? {
                if let Some(l) = self.live.get_mut(i)
                    && !l.external
                {
                    l.outbox.insert(key, r);
                }
                progress = true;
            }
        }
        Ok(progress)
    }

    fn offer(&mut self, i: &str, atom: &Json) -> Result<Offered, KernelError> {
        let key = atom["key"].as_str().unwrap_or_default().to_owned();
        if let Some(seq) = self.index.journals[i].keys.get(&key) {
            let record = self.index.journals[i].records[*seq as usize].clone();
            if !matches_atom(&record, atom) {
                return Ok(Offered::Diverged(format!(
                    "键 {key} 的记录与报出的 Atom 不同"
                )));
            }
            return Ok(Offered::Result(self.result_of(i, &record)?));
        }
        if let Some(f) = self.live.get(i).and_then(|live| live.inflight.get(&key)) {
            if !json_eq(&f.atom, atom) {
                return Ok(Offered::Diverged(format!(
                    "键 {key} 在途的 Atom 与报出的不同"
                )));
            }
            return Ok(match self.attempt(i, &key)? {
                Some(r) => Offered::Result(r),
                None => Offered::Pending,
            });
        }
        if self.index.journals[i].paused {
            return Ok(Offered::Pending);
        }
        let live = self.live.get_mut(i).expect("调用方查过");
        live.inflight.insert(
            key.clone(),
            Flight {
                atom: atom.clone(),
                generation: live.generation,
                accepted: None,
            },
        );
        Ok(match self.attempt(i, &key)? {
            Some(result) => Offered::Result(result),
            None => Offered::Pending,
        })
    }

    fn attempt(&mut self, i: &str, key: &str) -> Result<Option<Json>, KernelError> {
        let Some(mut flight) = self.live.get_mut(i).and_then(|l| l.inflight.remove(key)) else {
            return Ok(None);
        };
        let blocked = self.stopped.contains_key(i) || self.index.journals[i].paused;
        if blocked && !flight.world_effect() {
            if self.index.journals[i].paused {
                if let Some(execution) = flight.accepted.as_mut().and_then(|a| a.execution.take()) {
                    execution.discard();
                }
                flight.accepted = None;
            }
            self.live
                .get_mut(i)
                .unwrap()
                .inflight
                .insert(key.into(), flight);
            return Ok(None);
        }
        let atom = flight.atom.clone();
        let generation = flight.generation;
        let decision = if let Some(send) = atom.get("send") {
            self.decide_send(i, generation, key, send, &mut flight)
        } else {
            self.decide_receive(i, generation, key, &atom["receive"])
        };
        let plan = match decision {
            Ok(Some(plan)) => plan,
            other => {
                self.live
                    .get_mut(i)
                    .unwrap()
                    .inflight
                    .insert(key.into(), flight);
                return other.map(|_| None);
            }
        };
        if let Err(error) = self.commit(plan) {
            if let Some(live) = self.live.get_mut(i) {
                live.inflight.insert(key.into(), flight);
            }
            return Err(error);
        }
        if let Some(a) = flight.accepted.as_ref() {
            if a.channel == Some(Channel::Isolated) {
                if let Err(error) = a.port.as_ref().unwrap().retire(&a.attempt) {
                    self.stop(i, format!("committed attempt cleanup: {error}"))?;
                    return Err(host_error(error));
                }
                self.journal.retire_attempt(&a.attempt)?;
            }
        }
        if self.stopped.contains_key(i) {
            let pending = self.live[i].inflight.values().any(Flight::world_effect);
            self.stopped_world_effect.insert(i.to_owned(), pending);
            self.journal.set_stopped_world_effect(i, pending)?;
            if !pending && self.live.get(i).is_some_and(|l| l.external) {
                self.journal.resume_stopped(i)?;
                self.stopped.remove(i);
                self.stopped_world_effect.remove(i);
            }
        }
        let seq = self.index.journals[i].keys[key];
        let record = self.index.journals[i].records[seq as usize].clone();
        Ok(Some(self.result_of(i, &record)?))
    }

    // ---- 决定 ----

    fn decide_receive(
        &mut self,
        i: &str,
        g: u64,
        key: &str,
        filter: &Json,
    ) -> Result<Option<Plan>, KernelError> {
        let j = &self.index.journals[i];
        let list = |field: &str| filter.get(field).and_then(Json::as_array).cloned();
        let (ports, senders, vias) = (
            list("ports").unwrap_or_default(),
            list("sender"),
            list("via"),
        );
        let found = j.untaken.iter().copied().find(|n| {
            let (_, a) = what(&j.records[*n as usize]);
            ports.contains(&a["port"])
                && senders.as_ref().is_none_or(|s| s.contains(&a["sender"]))
                && vias.as_ref().is_none_or(|v| v.contains(&a["via"]))
        });
        let result = match found {
            Some(n) => json!(n),
            None if filter["timeout"] == 0 => Json::Null,
            None => return Ok(None),
        };
        let record = self.record(
            "self",
            None,
            json!({"receive": {"key": key, "filter": filter, "result": result}}),
        );
        Ok(Some(Plan {
            writes: vec![Write::Step(i.to_owned(), g, record)],
            effects: Vec::new(),
        }))
    }

    /// 按目标的标记解析：port 按那个 Required port 现在接上的，handle 按自己的 c-list。
    fn resolve(&self, i: &str, target: &Json) -> Option<String> {
        let j = &self.index.journals[i];
        let h = if let Some(port) = target.get("port").and_then(Json::as_str) {
            j.wire.get(port)?.clone()
        } else {
            target.get("handle")?.as_str()?.to_owned()
        };
        (j.holds(&h) && !self.view().revoked(&h)).then_some(h)
    }

    fn decide_send(
        &mut self,
        i: &str,
        g: u64,
        key: &str,
        send: &Json,
        flight: &mut Flight,
    ) -> Result<Option<Plan>, KernelError> {
        let value = &send["delivery"]["value"];
        let dependencies = strings(&send["delivery"]["dependencies"]);
        let value_id = match self.store.put_json(value) {
            Ok(id) => id,
            Err(StoreError::Value(e)) => {
                // 交出的值不是 Value：Language 的 bug，与重放分歧一样停下，不记。
                self.stop(i, format!("交出的值不是 Value：{e}"))?;
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };
        if flight.accepted.is_none() {
            let reference = self.resolve(i, &send["to"]);
            let refs = send["attach"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, target)| (name.clone(), self.resolve(i, target)))
                .collect();
            let provider = reference.as_deref().and_then(|r| self.view().provider(r));
            let chain = reference
                .as_deref()
                .and_then(|r| self.view().chain(r))
                .unwrap_or_default();
            flight.accepted = Some(AcceptedSend {
                reference,
                refs,
                provider,
                chain,
                channel: None,
                execution: None,
                completed: None,
                content: None,
                port: None,
                attempt: String::new(),
                consumed: false,
                atom: json!({"key":key,"send":send}),
                started: false,
            });
        }
        let accepted = flight.accepted.as_mut().unwrap();
        let r = accepted.reference.clone();
        let attach: serde_json::Map<String, Json> = accepted
            .refs
            .iter()
            .map(|(name, h)| (name.clone(), json!({"to":send["attach"][name],"ref":h})))
            .collect();
        let event = self.event(&[i, key]);
        let mut body = json!({"key": key, "to": send["to"], "ref": r, "delivery":{"value": value_id.as_str(),"dependencies": dependencies}, "attach": attach});
        let outcome = if r.is_some() && accepted.refs.values().all(Option::is_some) {
            let refs = accepted
                .refs
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().unwrap()))
                .collect();
            match self.dispatch(i, key, value, &dependencies, &refs, accepted)? {
                Some(o) => o,
                None => {
                    if accepted.execution.is_none() {
                        flight.accepted = None;
                    }
                    return Ok(None);
                }
            }
        } else {
            Outcome::rejected("denied", "目标或附带解析不到，或已被撤销")
        };
        self.store.put_json(&outcome.outcome)?;
        let result = json!({"outcome": outcome.outcome, "attach": outcome.attach, "derived": outcome.derived,"accepted":accepted.consumed});
        body["result"] = result;
        let caused = !outcome.caused.is_empty() || !outcome.creates.is_empty();
        let ev = caused.then_some(event.as_str());
        let step = self.record("self", ev, json!({"send": body}));
        let mut writes = vec![Write::Step(i.to_owned(), g, step)];
        for (target, mut rec) in outcome.creates {
            rec.at = (self.config.clock)();
            rec.event = Some(event.clone());
            writes.push(Write::Create(target, rec));
        }
        for (target, mut rec) in outcome.caused {
            rec.at = (self.config.clock)();
            rec.event = Some(event.clone());
            // 自己的一步在自己这本里造成的记录，by 是 self。
            if target == i {
                rec.by = "self".into();
            }
            writes.push(Write::Caused(target, rec));
        }
        Ok(Some(Plan {
            writes,
            effects: outcome.effects,
        }))
    }

    /// 结果按所经的链上每一节的 out 查；不满足的，是提供者交出的结果不对。
    fn checked(&self, mut o: Outcome, chain: &[Node]) -> Outcome {
        let bad = if validate(outcome_schema(), &o.outcome).is_err() {
            Some("提供者给出的不是结束状态".to_owned())
        } else if o.outcome["ok"] == true {
            check_out(&self.view(), chain, &o.outcome["value"], &o.attach)
        } else {
            None
        };
        if let Some(message) = bad {
            let dependencies = o.outcome["dependencies"].clone();
            o = Outcome::fail("use", message);
            o.outcome["dependencies"] = dependencies;
        }
        o
    }

    /// 派生出附带的那一份与它的撤销权。
    fn derive(&self, i: &str, key: &str, name: &str) -> Json {
        json!({
            "handle": self.handle(&[i, key, "derived", name]),
            "revoker": {"handle": self.handle(&[i, key, "revoker", name]), "spec": kernel_spec("revoke").0}
        })
    }

    /// 接收方为附带写的声明要与派生后链上另外某一节相同，或者写的是不限。`own` 是接收方
    /// 那一节在所经的链上的位置：交到 Provided port 时它就是链的第一节，比的是除它以外的。
    fn wire_ok(
        &self,
        parent: &str,
        via: &str,
        name: &str,
        extra: Option<Node>,
        own: Option<usize>,
    ) -> bool {
        let Some(extra) = extra else { return true };
        let view = self.view();
        let (Some(mut chain), Some(via_chain)) = (view.chain(parent), view.chain(via)) else {
            return false;
        };
        for (n, node) in via_chain.iter().enumerate() {
            if Some(n) == own {
                continue;
            }
            match view.decl(node, name) {
                Some(Some(d)) => chain.push(d),
                Some(None) => {}
                None => return false,
            }
        }
        chain
            .iter()
            .any(|n| view.same_node(n, &extra) || view.narrow_node(&extra, n))
    }

    /// 持有指向自己、或包含自己的 Collective 的 Correct：Kernel 拒绝。
    fn correct_of(&self, handle: &str, receiver: &str) -> bool {
        self.view().provider(handle) == Some(Provider::Correct(receiver.to_owned()))
    }

    /// 带 Seat 路径的 Look 与 Notify 落到哪个 Instance：沿路径逐段找占着那个 Seat 的成员，
    /// 嵌套时接着往下。不带路径的就是目标自己。见 docs/Kernel/Instance.md 的「成员的身份」。
    fn seat_target(&self, target: &str, value: &Json) -> Result<String, Outcome> {
        let Some(path) = value.get("seat").and_then(Json::as_str) else {
            return Ok(target.to_owned());
        };
        let view = self.view();
        let mut at = target.to_owned();
        for seat in path.split('/') {
            match view.member_of(&at, seat) {
                Some(m) => at = m,
                None => {
                    return Err(Outcome::fail(
                        "denied",
                        format!("路径 {path} 上没有 Seat {seat}"),
                    ));
                }
            }
        }
        Ok(at)
    }

    fn look(&self, target: &str, value: &Json) -> Result<Outcome, KernelError> {
        let target = match self.seat_target(target, value) {
            Ok(t) => t,
            Err(o) => return Ok(o),
        };
        let target = target.as_str();
        let j = &self.index.journals[target];
        match value["op"].as_str() {
            Some("inspect") => {
                let (status, outcome) = match j.ended {
                    Some(n) => ("ended", what(&j.records[n as usize]).1.clone()),
                    None if j.paused => ("paused", Json::Null),
                    None => ("running", Json::Null),
                };
                let mut result = Outcome::ok(json!({"status": status, "outcome": outcome}));
                if let Some(n) = j.ended {
                    result.outcome["dependencies"] =
                        json!(self.view().record_scope(target, n).bundles);
                }
                Ok(result)
            }
            _ => {
                let from = value["from"].as_u64().unwrap_or(0) as usize;
                let limit = value["limit"].as_u64().map_or(usize::MAX, |n| n as usize);
                let records: Vec<Json> = j.records.iter().skip(from).take(limit).cloned().collect();
                let mut roots = BTreeSet::new();
                for record in &records {
                    self.store.put_json(record)?;
                    roots.extend(
                        self.view()
                            .record_scope(target, record["seq"].as_u64().unwrap())
                            .bundles,
                    );
                }
                let mut result = Outcome::ok(json!({"records": records}));
                result.outcome["dependencies"] = json!(roots);
                Ok(result)
            }
        }
    }

    fn notify(
        &mut self,
        i: &str,
        key: &str,
        via: &str,
        target: &str,
        value: &Json,
        refs: &BTreeMap<String, String>,
    ) -> Result<Outcome, KernelError> {
        let target = match self.seat_target(target, value) {
            Ok(t) => t,
            Err(o) => return Ok(o),
        };
        let target = target.as_str();
        let to = &refs["to"];
        // 接下时就查：to 链上每一节都要收得下结束状态。
        let view = self.view();
        let Some(chain) = view.derive_chain(to, via, "to", None) else {
            return Ok(Outcome::rejected("denied", "to 的派生链解析不到"));
        };
        let outcome = strip_desc(outcome_schema());
        for node in &chain {
            let Some(port) = view.port_json(node) else {
                return Ok(Outcome::rejected("denied", "to 的一节解析不到"));
            };
            let input = strip_desc(&port["in"]);
            let fits = input.as_object().is_some_and(serde_json::Map::is_empty)
                || json_eq(&input, &outcome)
                || input["anyOf"]
                    .as_array()
                    .is_some_and(|b| b.iter().any(|x| json_eq(x, &outcome)));
            if !fits {
                return Ok(Outcome::rejected("type", "to 链上有一节收不下结束状态"));
            }
        }
        let d = self.derive(i, key, "to");
        let mut o = Outcome::ok(Json::Null);
        let handle = d["handle"].as_str().unwrap_or_default().to_owned();
        o.derived = json!({"to": d});
        let tj = &self.index.journals[target];
        if let Some(end) = tj.ended {
            let outcome_id = what(&tj.records[end as usize]).1.clone();
            if let Some(Provider::Provided {
                instance: recv,
                port,
            }) = view.provider(to)
                && self.index.journal(&recv).is_some_and(|j| j.ended.is_none())
            {
                let value_id = self.store.put_json(&outcome_id)?;
                let by = if recv == i {
                    "self".to_owned()
                } else {
                    i.to_owned()
                };
                let route = self.route_new(&handle, to, i, &recv);
                o.caused.push((recv, raw(&by, json!({"arrival": {"port": port, "sender": target, "via": route, "delivery":{"value":value_id.as_str(),"dependencies":outcome_id["dependencies"]}, "attach": {}}}))));
            }
        }
        Ok(o)
    }

    /// 还没提交的派生出的 Handle 的来路：它自己是接收方派生的，就是它；否则沿父往上找。
    fn route_new(&self, new: &str, parent: &str, deriver: &str, receiver: &str) -> String {
        if deriver == receiver {
            return new.to_owned();
        }
        let up = self.view().route(parent, receiver);
        if matches!(self.index.mints.get(&up), Some(Mint::Derived { at, .. }) if at.0 == receiver) {
            up
        } else {
            new.to_owned()
        }
    }

    fn correct(
        &mut self,
        i: &str,
        key: &str,
        via: &str,
        target: &str,
        value: &Json,
        refs: &BTreeMap<String, String>,
    ) -> Result<Outcome, KernelError> {
        let tj = &self.index.journals[target];
        if tj.ended.is_some() {
            return Ok(Outcome::rejected("denied", "目标已经结束，没有可纠正的"));
        }
        let op = value["op"].as_str().unwrap_or_default();
        let mut o = Outcome::ok(Json::Null);
        let rec = match op {
            "pause" | "resume" => json!({"correct": {"op": op}}),
            "grant" | "withdraw" => {
                let port = value["port"].as_str().unwrap_or_default();
                let def = self.view().definition(target).unwrap_or(Json::Null);
                if def["interface"]["requires"].get(port).is_none() {
                    return Ok(Outcome::rejected(
                        "type",
                        format!("目标没有 Required port {port}"),
                    ));
                }
                if op == "withdraw" {
                    json!({"correct": {"op": "withdraw", "port": port}})
                } else {
                    let Some(parent) = refs.get("ref") else {
                        return Ok(Outcome::rejected("type", "grant 要带 ref"));
                    };
                    if self.correct_of(parent, target) {
                        return Ok(Outcome::rejected(
                            "denied",
                            "不能让一个 Instance 持有指向自己的 Correct",
                        ));
                    }
                    let extra = View::spec_node_pub(&def, "requires", port, target);
                    if !self.wire_ok(parent, via, "ref", extra, None) {
                        return Ok(Outcome::rejected(
                            "type",
                            "ref 与目标的 Required port 的 Port spec 不同",
                        ));
                    }
                    let d = self.derive(i, key, "ref");
                    let h = d["handle"].clone();
                    o.derived = json!({"ref": d});
                    json!({"correct": {"op": "grant", "port": port, "ref": h}})
                }
            }
            _ => return Ok(Outcome::rejected("type", "不认识的 op")),
        };
        o.caused.push((target.to_owned(), raw(i, rec)));
        Ok(o)
    }

    fn revoke(&self, i: &str, d: &str) -> Outcome {
        let view = self.view();
        let mut o = Outcome::ok(Json::Null);
        if view.revoked(d) {
            return o;
        }
        let gone: BTreeSet<String> = std::iter::once(d.to_owned())
            .chain(
                self.index
                    .mints
                    .keys()
                    .filter(|h| view.ancestors(h).iter().any(|a| a == d))
                    .cloned(),
            )
            .collect();
        let mut caused = Vec::new();
        for (holder, j) in &self.index.journals {
            if j.ended.is_some() {
                continue;
            }
            let mut hit: Vec<(u64, String)> = gone
                .iter()
                .filter(|h| j.holds(h))
                .map(|h| (j.since.get(h).copied().unwrap_or(0), h.clone()))
                .collect();
            hit.sort();
            if !hit.is_empty() {
                caused.push((
                    holder.clone(),
                    hit.into_iter().map(|(_, h)| h).collect::<Vec<_>>(),
                ));
            }
        }
        for (holder, hs) in caused {
            o.caused.push((holder, raw(i, json!({"invalid": hs}))));
        }
        o
    }

    #[allow(clippy::too_many_arguments)]
    fn deliver(
        &mut self,
        i: &str,
        key: &str,
        r: &str,
        target: &str,
        port: &str,
        value: &Json,
        dependencies: &[String],
        refs: &BTreeMap<String, String>,
    ) -> Result<Outcome, KernelError> {
        if self.index.journal(target).is_none_or(|j| j.ended.is_some()) {
            return Ok(Outcome::rejected("denied", "交给的 Instance 已经结束"));
        }
        let mut o = Outcome::ok(Json::Null);
        let mut derived = serde_json::Map::new();
        let mut incoming = serde_json::Map::new();
        for (name, parent) in refs {
            if self.correct_of(parent, target) {
                return Ok(Outcome::rejected(
                    "denied",
                    "不能让一个 Instance 持有指向自己的 Correct",
                ));
            }
            let view = self.view();
            let receiver = view
                .chain(r)
                .and_then(|c| c.first().cloned())
                .and_then(|n| view.decl(&n, name).flatten());
            if !self.wire_ok(parent, r, name, receiver, Some(0)) {
                return Ok(Outcome::rejected(
                    "type",
                    format!("附带 {name} 不合接收方写的声明"),
                ));
            }
            let d = self.derive(i, key, name);
            let spec = view
                .source_spec(parent)
                .and_then(|id| view.load(&id))
                .unwrap_or(Json::Null);
            incoming.insert(name.clone(), json!({"handle": d["handle"], "spec": spec}));
            derived.insert(name.clone(), d);
        }
        o.derived = Json::Object(derived);
        let value_id = self.store.put_json(value)?;
        let route = self.view().route(r, target);
        o.caused.push((target.to_owned(), raw(i, json!({"arrival": {"port": port, "sender": i, "via": route, "delivery":{"value":value_id.as_str(),"dependencies":dependencies}, "attach": incoming}}))));
        Ok(o)
    }

    fn decide_end(&mut self, i: &str, done: &Json) -> Result<Plan, KernelError> {
        let view = self.view();
        let def = view.definition(i).unwrap_or(Json::Null);
        let mut outcome = done.clone();
        if outcome["ok"] == true
            && validate(&def["interface"]["signature"]["out"], &outcome["value"]).is_err()
        {
            let dependencies = outcome["dependencies"].clone();
            outcome = failure("type", "结果不合 Signature 的 out", Json::Null);
            outcome["dependencies"] = dependencies;
        }
        if validate(outcome_schema(), &outcome).is_err() {
            outcome = failure("type", "done 不是结束状态", Json::Null);
        }
        let access = Content::new(
            self.store.clone(),
            self.view().scope(i, None).map_err(host_error)?,
            self.config.budgets,
        )
        .map_err(host_error)?;
        if !access.grants(&outcome["dependencies"]) {
            outcome = failure("denied", "done dependencies not acquired", Json::Null);
        }
        let outcome_id = self.store.put_json(&outcome)?;
        let mut caused: Vec<(String, NewRecord)> = Vec::new();
        let view = self.view();
        for ((reg, _), to) in view.registrations(i) {
            if let Some(Provider::Provided {
                instance: recv,
                port,
            }) = view.provider(&to)
                && recv != i
                && self.index.journal(&recv).is_some_and(|j| j.ended.is_none())
            {
                let route = view.route(&to, &recv);
                let _ = reg;
                caused.push((recv, raw(i, json!({"arrival": {"port": port, "sender": i, "via": route, "delivery":{"value":outcome_id.as_str(),"dependencies":outcome["dependencies"]}, "attach": {}}}))));
            }
        }
        for (holder, j) in &self.index.journals {
            if holder == i || j.ended.is_some() {
                continue;
            }
            let mut hit: Vec<(u64, String)> = j
                .since
                .iter()
                .filter(|(h, _)| j.holds(h) && matches!(view.provider(h), Some(Provider::Provided { instance, .. }) if instance == i))
                .map(|(h, s)| (*s, h.clone()))
                .collect();
            hit.sort();
            if !hit.is_empty() {
                caused.push((
                    holder.clone(),
                    raw(
                        i,
                        json!({"invalid": hit.into_iter().map(|(_, h)| h).collect::<Vec<_>>()}),
                    ),
                ));
            }
        }
        // 同一本 Journal 里先写到达，再写失效；本之间按身份排，Journal 才确定。
        caused.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| rank(&a.1).cmp(&rank(&b.1))));
        let event = (!caused.is_empty()).then(|| self.event(&[i, "end"]));
        let end = self.record("self", event.as_deref(), json!({"end": outcome}));
        let g = self.live.get(i).map_or(0, |l| l.generation);
        let mut writes = vec![Write::Step(i.to_owned(), g, end)];
        for (target, mut rec) in caused {
            rec.at = (self.config.clock)();
            rec.event = event.clone();
            writes.push(Write::Caused(target, rec));
        }
        Ok(Plan {
            writes,
            effects: vec![Effect::Ended(i.to_owned())],
        })
    }
}

fn discard(mut flight: Flight) {
    if let Some(execution) = flight.accepted.as_mut().and_then(|a| a.execution.take()) {
        execution.discard();
    }
}

enum Offered {
    Result(Json),
    Pending,
    Diverged(String),
}

fn host_error(e: croon_port::HostError) -> KernelError {
    KernelError::Host(e.to_string())
}
fn canonical_dependencies(v: &Json) -> bool {
    let Some(array) = v.as_array() else {
        return false;
    };
    let Some(strings) = array.iter().map(Json::as_str).collect::<Option<Vec<_>>>() else {
        return false;
    };
    strings.iter().all(|id| ValueId::parse(id).is_some()) && strings.windows(2).all(|w| w[0] < w[1])
}
fn strings(v: &Json) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .map(str::to_owned)
        .collect()
}

fn decode_base64(s: &str) -> Option<Vec<u8>> {
    if s.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::new();
    for (index, c) in s.as_bytes().chunks(4).enumerate() {
        let decode = |c: u8| -> Option<u32> {
            Some(match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a' + 26) as u32,
                b'0'..=b'9' => (c - b'0' + 52) as u32,
                b'+' => 62,
                b'/' => 63,
                _ => return None,
            })
        };
        let a = decode(c[0])?;
        let b = decode(c[1])?;
        let last = index + 1 == s.len() / 4;
        if c[2] == b'=' {
            if !last || c[3] != b'=' || b & 15 != 0 {
                return None;
            }
            out.push(((a << 2) | (b >> 4)) as u8);
        } else {
            let d = decode(c[2])?;
            out.push(((a << 2) | (b >> 4)) as u8);
            out.push(((b << 4) | (d >> 2)) as u8);
            if c[3] == b'=' {
                if !last || d & 3 != 0 {
                    return None;
                }
            } else {
                out.push(((d << 6) | decode(c[3])?) as u8)
            }
        }
    }
    Some(out)
}

fn rank(r: &NewRecord) -> u8 {
    if r.what.get("arrival").is_some() {
        0
    } else {
        1
    }
}

/// 别处造成的一条记录；`at` 与事件在决定的最后补上。
fn raw(by: &str, what: Json) -> NewRecord {
    NewRecord {
        at: String::new(),
        by: by.to_owned(),
        event: None,
        what,
    }
}

fn strip_desc(v: &Json) -> Json {
    crate::index::strip(v)
}

/// 交出的 Value 与附带按链上每一节查。失败记在交出的一方：违反的节里有它自己写的，是 type；
/// 只违反别人加的，是 denied。它没为这份 Reference 写过的，提供者那一节算它写的。
fn check_in(
    view: &View,
    sender: &str,
    chain: &[Node],
    value: &Json,
    names: &BTreeSet<String>,
) -> Option<(String, String)> {
    let mut violated = Vec::new();
    let mut first = None;
    for (n, node) in chain.iter().enumerate() {
        let Some(port) = view.port_json(node) else {
            violated.push(n);
            continue;
        };
        let mut bad = validate(&port["in"], value).err().map(|v| {
            v.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("；")
        });
        let attach = &port["attach"];
        for name in names {
            let leaf = attach
                .get("properties")
                .and_then(|p| p.get(name))
                .or_else(|| attach.get("additionalProperties"));
            if matches!(leaf, None | Some(Json::Bool(false))) {
                bad.get_or_insert_with(|| format!("不接受附带 {name}"));
            }
        }
        for req in attach
            .get("required")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            if req.as_str().is_some_and(|r| !names.contains(r)) {
                bad.get_or_insert_with(|| format!("缺少附带 {req}"));
            }
        }
        if let Some(b) = bad {
            first.get_or_insert(b);
            violated.push(n);
        }
    }
    if violated.is_empty() {
        return None;
    }
    let wrote_any = chain.iter().any(|n| n.author.as_deref() == Some(sender));
    let mine = violated
        .iter()
        .any(|&n| chain[n].author.as_deref() == Some(sender) || (!wrote_any && n == 0));
    Some((
        if mine { "type" } else { "denied" }.to_owned(),
        first.unwrap_or_default(),
    ))
}

/// 结果按链上每一节的 out 查；没有 out 的节不约束结果。
fn check_out(view: &View, chain: &[Node], value: &Json, attach: &Json) -> Option<String> {
    for node in chain {
        let Some((root, ptr)) = view.port_at(node) else {
            return Some("结果契约的一节解析不到".into());
        };
        let port = root.pointer(&ptr).unwrap();
        let Some(out) = port.get("out") else { continue };
        if let Err(v) = validate(&out["value"], value) {
            return Some(format!(
                "提供者给出的结果不合 out：{}",
                v.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("；")
            ));
        }
        let decl = &out["attach"];
        for req in decl["required"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
        {
            if attach.get(req).is_none() {
                return Some(format!("提供者的结果缺少附带 {req}"));
            }
        }
        for (name, incoming) in attach.as_object().into_iter().flatten() {
            let (leaf, path) = match decl.get("properties").and_then(|p| p.get(name)) {
                Some(leaf) => (
                    leaf,
                    format!(
                        "{ptr}/out/attach/properties/{}",
                        name.replace('~', "~0").replace('/', "~1")
                    ),
                ),
                None => (
                    &decl["additionalProperties"],
                    format!("{ptr}/out/attach/additionalProperties"),
                ),
            };
            match leaf {
                Json::Bool(true) => {}
                Json::Object(_) => {
                    let mut declared = Node {
                        spec: node.spec.clone(),
                        ptr: path,
                        author: node.author.clone(),
                    };
                    if let Some(r) = leaf["$ref"].as_str() {
                        declared.ptr = format!(
                            "/$defs/{}",
                            r.strip_prefix("#/$defs/")
                                .unwrap_or_default()
                                .replace('~', "~0")
                                .replace('/', "~1")
                        );
                    }
                    let actual = Node {
                        spec: ValueId::of_json(&incoming["spec"])
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                        ptr: String::new(),
                        author: None,
                    };
                    if !view.same_node(&actual, &declared) {
                        return Some(format!("结果附带 {name} 的实际规格不合 out 声明"));
                    }
                }
                _ => return Some(format!("提供者的结果带来了不收的 {name}")),
            }
        }
    }
    None
}

/// 报出的 Atom 与键上已有的记录是不是同一个。
fn matches_atom(record: &Json, atom: &Json) -> bool {
    let (kind, body) = what(record);
    match (kind, atom.get("send"), atom.get("receive")) {
        ("send", Some(send), _) => {
            let same_attach = send["attach"].as_object().is_some_and(|a| {
                let recorded = body["attach"].as_object();
                recorded.is_some_and(|r| {
                    r.len() == a.len()
                        && a.iter()
                            .all(|(k, t)| r.get(k).is_some_and(|x| json_eq(&x["to"], t)))
                })
            });
            json_eq(&body["to"], &send["to"])
                && ValueId::of_json(&send["delivery"]["value"])
                    .is_ok_and(|id| body["delivery"]["value"] == id.as_str())
                && json_eq(
                    &body["delivery"]["dependencies"],
                    &send["delivery"]["dependencies"],
                )
                && same_attach
        }
        ("receive", _, Some(receive)) => json_eq(&body["filter"], receive),
        _ => false,
    }
}

/// 一个系统的密钥：第一次打开时生成，之后一直用它，ID 才撑得过重启。
fn system_key(dir: &Path) -> Result<Vec<u8>, KernelError> {
    let path = dir.join("system.key");
    let io = |e: std::io::Error| KernelError::Refused(format!("{}：{e}", path.display()));
    if let Ok(key) = std::fs::read(&path) {
        return Ok(key);
    }
    use std::io::{Read as _, Write as _};
    let mut key = vec![0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut key))
        .map_err(io)?;
    let tmp = dir.join("system.key.tmp");
    let mut f = std::fs::File::create(&tmp).map_err(io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(io)?;
    }
    f.write_all(&key).map_err(io)?;
    f.sync_all().map_err(io)?;
    std::fs::rename(&tmp, &path).map_err(io)?;
    Ok(key)
}
