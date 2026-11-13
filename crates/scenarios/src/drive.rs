//! 驱动：拿一个场景驱动真的 Kernel，按与预期的偏序相容的某个全序推进，每一步都与预期比对。
//! 参考：docs/Kernel/Scenarios/README.md（脚手架、扰动）；docs/Kernel/Journal.md（实现）。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::blocks;
use kernel::host::{Channel, Config, Fault, Language};
use kernel::index::{Index, what};
use kernel::kernel::{Conn, Kernel, KernelError};
use kernel::schema::json_eq;
use kernel::store::Budgets;
use serde_json::{Value as Json, json};

use crate::bind::Bind;
use crate::expand::{self, Made};
use crate::fake::{FakeLang, FakePort, Shared, atom_of};
use crate::scenario::{Scn, Step};

/// 一组场景共用的东西：展开过的 Value、展开时造出的内容，以及 `fake@1` 之外要装的 Language。
pub struct Setup {
    /// 测试系统的密钥：固定同次取得的 Handle 的排序；None 让 Kernel 正常生成。
    pub system_key: Option<Vec<u8>>,
    pub values: BTreeMap<String, Json>,
    pub made: Made,
    /// 可在任何授权发生前预存的 Value；预存不授予能读的资格。
    pub preload: Vec<String>,
    pub languages: fn() -> Vec<Box<dyn Language>>,
}

impl Setup {
    /// `values` 是还没展开的共用名字空间。
    pub fn new(
        values: &BTreeMap<String, Json>,
        languages: fn() -> Vec<Box<dyn Language>>,
    ) -> Setup {
        let (values, made) = expand::values(values);
        Setup {
            values,
            made,
            preload: Vec::new(),
            languages,
            system_key: Some(vec![7; 32]),
        }
    }

    /// 只装 `fake@1`。
    pub fn fake(values: &BTreeMap<String, Json>) -> Setup {
        Setup::new(values, Vec::new)
    }

    /// 驱动一次：`seed` 选全序，`at` 是在第几个事务处退出、是否在提交之后。
    pub fn run(&self, scn: &Rc<Scn>, seed: u64, at: Option<(usize, bool)>) -> Run {
        let mut r = Run::new(scn.clone(), self, seed, at);
        r.drive();
        r
    }
}

// ---- 故障 ----

#[derive(Default)]
pub struct Faults {
    pub commits: usize,
    /// 第几个事务处退出；真是在提交之后。
    at: Option<(usize, bool)>,
    /// 这个键的 Atom 在途、结果提交之前退出，各一次。
    keys: Vec<(String, String, bool)>,
}

impl Faults {
    fn hit(&mut self, f: &Fault, bind: &Bind) -> bool {
        match f {
            Fault::BeforeCommit { records } => {
                self.commits += 1;
                if self.at == Some((self.commits, false)) {
                    return true;
                }
                for (ph, key, done) in &mut self.keys {
                    let actual = bind.fwd.get(ph.as_str());
                    if !*done
                        && records
                            .iter()
                            .any(|(i, k)| Some(i) == actual && k.as_deref() == Some(key.as_str()))
                    {
                        *done = true;
                        return true;
                    }
                }
                false
            }
            Fault::AfterCommit { .. } => self.at == Some((self.commits, true)),
        }
    }
}

// ---- 驱动 ----

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(tag: &str) -> Dir {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "croon-scaffold-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    Dir(dir)
}

pub struct Run {
    pub scn: Rc<Scn>,
    pub shared: Rc<RefCell<Shared>>,
    pub faults: Rc<RefCell<Faults>>,
    dir: Dir,
    pub kernel: Option<Kernel>,
    conn: Option<Conn>,
    rng: u64,
    checked: BTreeMap<String, usize>,
    pub crashes: usize,
    done_perturb: BTreeSet<usize>,
    root_ph: String,
    made: Made,
    preload: Vec<String>,
    languages: fn() -> Vec<Box<dyn Language>>,
}

fn config(
    shared: &Rc<RefCell<Shared>>,
    faults: &Rc<RefCell<Faults>>,
    languages: fn() -> Vec<Box<dyn Language>>,
) -> Config {
    let (s, f) = (shared.clone(), faults.clone());
    let mut all: Vec<Box<dyn Language>> = vec![Box::new(FakeLang(shared.clone()))];
    all.extend(languages());
    Config {
        languages: all,
        budgets: Budgets {
            nodes: 100_000,
            read_bytes: 512 * 1024 * 1024,
            write_bytes: 512 * 1024 * 1024,
        },
        ports: BTreeMap::new(),
        clock: Box::new(|| "2026-01-01T00:00:00Z".to_owned()),
        fault: Some(Box::new(move |point| {
            f.borrow_mut().hit(point, &s.borrow().bind)
        })),
    }
}

/// 交给 llm 或 exec 的 Port spec。
fn provider_specs() -> [Json; 2] {
    [
        blocks::json("model.generate"),
        blocks::json("directory.exec"),
    ]
}

impl Run {
    fn new(scn: Rc<Scn>, setup: &Setup, seed: u64, at: Option<(usize, bool)>) -> Run {
        let real = scn
            .journals
            .iter()
            .filter(|(_, j)| {
                let d = what(&j[0]).1["definition"].as_str().unwrap_or_default();
                setup
                    .values
                    .get(d)
                    .and_then(|d| d["$bundle"]["language"].as_str())
                    .is_some_and(|l| l != "fake@1")
            })
            .map(|(ph, _)| ph.clone())
            .collect();
        let mut bind = Bind::new(setup.values.clone());
        bind.made.extend(setup.made.json.clone());
        bind.bytes.extend(setup.made.bytes.clone());
        let shared = Rc::new(RefCell::new(Shared {
            scn: scn.clone(),
            bind,
            allowed: None,
            real,
        }));
        let keys = scn
            .perturb
            .iter()
            .filter_map(|p| p.get("crash").filter(|c| c.get("key").is_some()))
            .map(|c| {
                (
                    c["instance"].as_str().unwrap().to_owned(),
                    c["key"].as_str().unwrap().to_owned(),
                    false,
                )
            })
            .collect();
        let faults = Rc::new(RefCell::new(Faults {
            commits: 0,
            at,
            keys,
        }));
        let dir = temp_dir(&scn.name);
        if let Some(key) = &setup.system_key {
            std::fs::create_dir_all(&dir.0).unwrap();
            std::fs::write(dir.0.join("system.key"), key).unwrap();
        }
        let kernel = Kernel::open(&dir.0, config(&shared, &faults, setup.languages)).unwrap();
        let root_ph = scn
            .journals
            .iter()
            .find(|(_, j)| j[0]["by"] == "outside")
            .map(|(p, _)| p.clone())
            .expect("有 root entry");
        Run {
            scn,
            shared,
            faults,
            dir,
            kernel: Some(kernel),
            conn: None,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            checked: BTreeMap::new(),
            crashes: 0,
            done_perturb: BTreeSet::new(),
            root_ph,
            made: setup.made.clone(),
            preload: setup.preload.clone(),
            languages: setup.languages,
        }
    }

    fn bind_hosts(&mut self) {
        let kernel = self.kernel.as_ref().unwrap();
        let Some(root) = kernel.index().root() else {
            return;
        };
        let view = kernel::index::View {
            ix: kernel.index(),
            store: kernel.store(),
        };
        let bindings: Vec<_> = kernel.index().journals[root].birth()["wire"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(_, h)| {
                let h = h.as_str()?;
                if view.provider(h) != Some(kernel::index::Provider::Host(h.into())) {
                    return None;
                }
                let (_, port) = kernel.index().birth_wire.get(h)?;
                let binding = &kernel.index().journals[root].birth()["binding"]["sources"][port];
                let spec = &binding["spec"];
                let channel =
                    if kernel::index::same_spec(spec, "", &blocks::json("directory.exec"), "") {
                        Channel::Isolated
                    } else {
                        Channel::Discardable
                    };
                Some((h.to_owned(), channel, binding["host"].as_str()?.to_owned()))
            })
            .collect();
        for (source, channel, identity) in bindings {
            let shared = self.shared.clone();
            self.k()
                .bind_host(
                    &source,
                    Rc::new(FakePort {
                        shared,
                        channel,
                        identity,
                    }),
                )
                .unwrap();
        }
    }

    fn k(&mut self) -> &mut Kernel {
        self.kernel.as_mut().unwrap()
    }

    fn reopen(&mut self) {
        self.kernel = None;
        self.conn = None;
        self.shared.borrow_mut().allowed = None;
        self.kernel = Some(
            Kernel::open(
                &self.dir.0,
                config(&self.shared, &self.faults, self.languages),
            )
            .unwrap(),
        );
        self.bind_hosts();
        self.crashes += 1;
    }

    /// Kernel 的调用：以 Crashed 失败就重开。
    fn guard<T>(&mut self, r: Result<T, KernelError>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(KernelError::Crashed) => {
                self.reopen();
                None
            }
            Err(e) => panic!("{}：{e}", self.scn.name),
        }
    }

    fn len_of(&self, ph: &str) -> usize {
        let s = self.shared.borrow();
        s.bind
            .fwd
            .get(ph)
            .and_then(|a| self.kernel.as_ref().unwrap().index().journal(a))
            .map_or(0, |j| j.records.len())
    }

    fn bound(&self, ph: &str) -> bool {
        self.shared.borrow().bind.fwd.contains_key(ph)
    }

    fn sync(&mut self) {
        let kernel = self.kernel.as_ref().unwrap();
        let mut s = self.shared.borrow_mut();
        loop {
            let mut changed = false;
            for (actual, j) in &kernel.index().journals {
                let ph = match s.bind.back.get(actual) {
                    Some(p) => p.clone(),
                    None => {
                        let r0 = &j.records[0];
                        let ph = if r0["by"] == "outside" {
                            self.root_ph.clone()
                        } else {
                            let Some(ev) = r0["event"]
                                .as_str()
                                .and_then(|e| s.bind.back.get(e))
                                .cloned()
                            else {
                                continue;
                            };
                            match self.scn.journals.iter().find(|(_, x)| {
                                x[0]["event"] == ev.as_str() && x[0]["what"].get("birth").is_some()
                            }) {
                                Some((p, _)) => p.clone(),
                                None => panic!("{}：{actual} 的诞生在预期里没有", self.scn.name),
                            }
                        };
                        s.bind
                            .bind(&ph, actual)
                            .unwrap_or_else(|e| panic!("{}：{e}", self.scn.name));
                        changed = true;
                        ph
                    }
                };
                let exp = &self.scn.journals[&ph];
                let from = self.checked.get(actual).copied().unwrap_or(0);
                for n in from..j.records.len() {
                    let e = exp.get(n).unwrap_or_else(|| {
                        panic!(
                            "{}：{ph} 多出第 {n} 条记录：{}",
                            self.scn.name, j.records[n]
                        )
                    });
                    if let Err(m) = s.bind.compare(e, &j.records[n], &format!("{ph}#{n}")) {
                        panic!("{}：{m}\n预期：{e}\n实际：{}", self.scn.name, j.records[n]);
                    }
                    changed = true;
                }
                self.checked.insert(actual.clone(), j.records.len());
            }
            if !changed {
                break;
            }
        }
        // 脚手架造的 Tree 都进内容库：exec 写出的后一棵也是。
        for c in s.bind.made.values() {
            kernel.import(c).unwrap();
        }
    }

    fn complete(&self) -> bool {
        self.scn
            .journals
            .iter()
            .all(|(ph, exp)| self.bound(ph) && self.len_of(ph) == exp.len())
    }

    fn is_real(&self, ph: &str) -> bool {
        self.shared.borrow().real.contains(ph)
    }

    /// 真的 Language 的这条交出交给 llm 或 exec：按它所经的 Port spec 判。目标是 Required port 的，
    /// 取诞生里的 Definition 写的；是 Handle 的，取它进来的那一处带着的 Port spec。
    pub fn to_provider(&self, ph: &str, send: &Json) -> bool {
        let s = self.shared.borrow();
        if let Some(actual) = send["ref"].as_str().and_then(|h| s.bind.fwd.get(h)) {
            let kernel = self.kernel.as_ref().unwrap();
            let view = kernel::index::View {
                ix: kernel.index(),
                store: kernel.store(),
            };
            return matches!(
                view.provider(actual),
                Some(kernel::index::Provider::Host(_))
            );
        }
        let exp = &self.scn.journals[ph];
        let spec = if let Some(port) = send["to"]["port"].as_str() {
            let d = what(&exp[0]).1["definition"].as_str().unwrap();
            s.bind.values[d]["$bundle"]["interface"]["requires"][port].clone()
        } else {
            let h = &send["to"]["handle"];
            let incoming = exp.iter().find_map(|r| {
                let (kind, body) = what(r);
                let attach = match kind {
                    "send" => body["result"]["attach"].as_object(),
                    "arrival" => body["attach"].as_object(),
                    _ => None,
                };
                attach
                    .into_iter()
                    .flatten()
                    .map(|(_, i)| i)
                    .find(|i| i["handle"] == *h)
                    .cloned()
            });
            match incoming.map(|i| i["spec"].clone()) {
                Some(spec) => spec,
                None => return false,
            }
        };
        provider_specs().iter().any(|p| json_eq(p, &spec))
    }

    /// 真的 Language 从第 `n` 条起的一步：这一次 llm 或 exec 的结果，加上同一本 Journal 里紧接着的、
    /// 由 Kernel 当场完成的那几条，到下一个取、下一个交给 llm 或 exec 的交出、或别人造成的记录为止。
    /// 每一条都要能走：它造成的记录正好是别的 Journal 的下一条，再加上 order。
    fn segment_enabled(&self, ph: &str, n: usize) -> bool {
        let exp = &self.scn.journals[ph];
        let mut lens: BTreeMap<&str, usize> = self
            .scn
            .journals
            .keys()
            .map(|p| (p.as_str(), self.len_of(p)))
            .collect();
        for (m, r) in exp.iter().enumerate().skip(n) {
            let (kind, body) = what(r);
            if m > n
                && (r["by"] != "self"
                    || kind == "receive"
                    || (kind == "send" && self.to_provider(ph, body)))
            {
                break;
            }
            if let Some(ev) = r.get("event") {
                for (ph2, exp2) in &self.scn.journals {
                    if ph2 == ph {
                        continue;
                    }
                    let Some(first) = exp2.iter().position(|r2| r2.get("event") == Some(ev)) else {
                        continue;
                    };
                    if lens[ph2.as_str()] != first {
                        return false;
                    }
                    let last = exp2
                        .iter()
                        .rposition(|r2| r2.get("event") == Some(ev))
                        .unwrap();
                    lens.insert(ph2, last + 1);
                }
            }
            for ((f, fi), (t, ti)) in &self.scn.order {
                if t == ph && *ti == m && lens[f.as_str()] <= *fi {
                    return false;
                }
            }
            lens.insert(ph, m + 1);
        }
        true
    }

    /// 现在能走的一步。`fake@1` 与 root entry：它是这本 Journal 的下一条，它在别处造成的记录正好是
    /// 那些 Journal 的下一条，再加上 order。真的 Language：下一条是交给 llm 或 exec 的交出，且整段能走。
    fn enabled(&self) -> Vec<Step> {
        let mut out = Vec::new();
        for (ph, exp) in &self.scn.journals {
            let n = self.len_of(ph);
            if n >= exp.len() || !self.bound(ph) {
                continue;
            }
            let r = &exp[n];
            let (kind, body) = what(r);
            if r["by"] != "self" || !matches!(kind, "send" | "receive" | "end") {
                continue;
            }
            if self.is_real(ph) {
                if kind == "send" && self.to_provider(ph, body) && self.segment_enabled(ph, n) {
                    out.push((ph.clone(), n));
                }
                continue;
            }
            let mut ok = true;
            if let Some(ev) = r.get("event") {
                for (ph2, exp2) in &self.scn.journals {
                    let first = exp2.iter().position(|r2| r2.get("event") == Some(ev));
                    if ph2 != ph && first.is_some_and(|m| self.len_of(ph2) != m) {
                        ok = false;
                    }
                }
            }
            for ((f, fi), (t, ti)) in &self.scn.order {
                if t == ph && *ti == n && self.len_of(f) <= *fi {
                    ok = false;
                }
            }
            if ok {
                out.push((ph.clone(), n));
            }
        }
        out
    }

    fn pick(&mut self, n: usize) -> usize {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % n as u64) as usize
    }

    fn connect(&mut self) -> Option<Conn> {
        if self.conn.is_none() {
            let root = self.shared.borrow().bind.fwd[&self.root_ph].clone();
            let c = self.k().connect(&root);
            self.conn = self.guard(c);
        }
        self.conn.clone()
    }

    fn create_root(&mut self) {
        let (definition, input, self_map, binding) = {
            let mut s = self.shared.borrow_mut();
            let birth = what(&self.scn.journals[&self.root_ph][0]).1.clone();
            let definition = s.bind.vid(birth["definition"].as_str().unwrap());
            let value = s.bind.content(birth["input"]["value"].as_str().unwrap());
            let mut dependencies = s.bind.subst(&birth["input"]["dependencies"]);
            if let Some(items) = dependencies.as_array_mut() {
                items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                items.dedup();
            }
            let binding = s.bind.subst(&birth["binding"]);
            (
                definition,
                json!({"value":value,"dependencies":dependencies}),
                birth["self"].clone(),
                binding,
            )
        };
        for id in &self.preload {
            self.shared.borrow_mut().bind.vid(id);
        }
        self.made.import(self.kernel.as_ref().unwrap().store());
        for c in self.shared.borrow().bind.made.values() {
            self.kernel.as_ref().unwrap().import(c).unwrap();
        }
        let r = self
            .k()
            .create_root(&definition, &input, &self_map, &binding);
        self.guard(r);
        self.bind_hosts();
    }

    fn root_step(&mut self, idx: usize) {
        let atom = {
            let mut s = self.shared.borrow_mut();
            atom_of(&mut s.bind, &self.scn.journals[&self.root_ph][idx])
        };
        let Some(conn) = self.connect() else { return };
        let r = self.k().submit(&conn, &atom);
        if let Some(None) = self.guard(r) {
            panic!("{}：root entry 的第 {idx} 步没有结果", self.scn.name);
        }
    }

    fn perturb(&mut self) {
        for (n, p) in self.scn.perturb.clone().iter().enumerate() {
            if self.done_perturb.contains(&n) {
                continue;
            }
            if let Some(r) = p.get("reconnect") {
                let after = r["after"].as_u64().unwrap() as usize;
                if self.len_of(&self.root_ph) <= after {
                    continue;
                }
                self.done_perturb.insert(n);
                let Some(old) = self.connect() else { continue };
                let root = self.shared.borrow().bind.fwd[&self.root_ph].clone();
                let new = self.k().connect(&root).unwrap();
                let atom_for = |run: &Run, key: &Json| {
                    let mut s = run.shared.borrow_mut();
                    let r = run.scn.journals[&run.root_ph]
                        .iter()
                        .find(|r| what(r).1["key"] == *key)
                        .expect("有这个键")
                        .clone();
                    atom_of(&mut s.bind, &r)
                };
                for key in r["resend"].as_array().unwrap() {
                    let before = self.len_of(&self.root_ph);
                    let atom = atom_for(self, key);
                    let got = self.k().submit(&new, &atom).unwrap();
                    assert!(
                        got.is_some(),
                        "{}：重报 {key} 要直接得到结果",
                        self.scn.name
                    );
                    assert_eq!(
                        self.len_of(&self.root_ph),
                        before,
                        "{}：重报 {key} 不重做",
                        self.scn.name
                    );
                }
                for key in r["stale"].as_array().unwrap() {
                    let before = self.len_of(&self.root_ph);
                    let atom = atom_for(self, key);
                    let got = self.k().submit(&old, &atom);
                    assert!(
                        matches!(got, Err(KernelError::Stale(_))),
                        "{}：旧会话报的 {key} 要被拒：{got:?}",
                        self.scn.name
                    );
                    assert_eq!(
                        self.len_of(&self.root_ph),
                        before,
                        "{}：被拒的 {key} 不记",
                        self.scn.name
                    );
                }
                self.conn = Some(new);
            }
            if let Some(c) = p.get("crash")
                && let Some(after) = c.get("after").and_then(Json::as_u64)
                && self.len_of(c["instance"].as_str().unwrap()) > after as usize
            {
                self.done_perturb.insert(n);
                self.reopen();
            }
        }
    }

    fn drive(&mut self) {
        for round in 0.. {
            assert!(round < 10_000, "{}：走不完", self.scn.name);
            if !self.bound(&self.root_ph) && self.k().index().root().is_none() {
                self.create_root();
                self.sync();
                continue;
            }
            let r = self.k().run();
            if self.guard(r).is_none() {
                continue;
            }
            self.sync();
            self.perturb();
            self.sync();
            if self.complete() {
                break;
            }
            let enabled = self.enabled();
            if enabled.is_empty() {
                let lens: Vec<_> = self
                    .scn
                    .journals
                    .keys()
                    .map(|p| (p.clone(), self.len_of(p)))
                    .collect();
                panic!("{}：没有能走的一步，停在 {lens:?}", self.scn.name);
            }
            let n = self.pick(enabled.len());
            let (ph, idx) = enabled[n].clone();
            if ph == self.root_ph {
                self.root_step(idx);
            } else {
                self.shared.borrow_mut().allowed = Some((ph.clone(), idx));
                let r = self.k().run();
                self.guard(r);
                self.shared.borrow_mut().allowed = None;
                if self.kernel.is_some() && self.crashes == 0 {
                    self.sync();
                    assert!(
                        self.len_of(&ph) > idx,
                        "{}：{ph} 的第 {idx} 步没有走出来",
                        self.scn.name
                    );
                }
            }
            self.sync();
        }
        // 最后再跑一轮：pending 里的 Atom 报出来，留在途中。
        let r = self.k().run();
        self.guard(r);
        self.sync();
        assert!(
            self.complete(),
            "{}：最后一轮之后 Journal 变了",
            self.scn.name
        );
        let kernel = self.kernel.as_ref().unwrap();
        for actual in kernel.index().journals.keys() {
            assert!(
                self.shared.borrow().bind.back.contains_key(actual),
                "{}：多出 Instance {actual}",
                self.scn.name
            );
        }
        for (ph, atoms) in &self.scn.pending {
            let actual = self.shared.borrow().bind.fwd[ph].clone();
            let flying: BTreeSet<String> = kernel.inflight(&actual).into_iter().collect();
            let want: BTreeSet<String> = atoms
                .iter()
                .map(|a| a["key"].as_str().unwrap().to_owned())
                .collect();
            assert!(
                flying.is_subset(&want),
                "{}：{ph} 在途的是 {flying:?}",
                self.scn.name
            );
            if self.crashes == 0 {
                assert_eq!(
                    flying, want,
                    "{}：{ph} 报出、到最后也没有记录的",
                    self.scn.name
                );
            }
        }
        assert!(
            kernel.stopped().is_empty(),
            "{}：有 Instance 被停下：{:?}",
            self.scn.name,
            kernel.stopped()
        );
        assert_eq!(
            self.done_perturb.len(),
            self.scn
                .perturb
                .iter()
                .filter(|p| p.get("reconnect").is_some() || p["crash"].get("after").is_some())
                .count(),
            "{}：扰动没有全做",
            self.scn.name
        );
        assert!(
            self.faults.borrow().keys.iter().all(|(_, _, done)| *done),
            "{}：在途的崩溃没有发生",
            self.scn.name
        );
        let rebuilt = Index::rebuild(kernel.journal().all().unwrap());
        assert!(
            *kernel.index() == rebuilt,
            "{}：索引不等于对 Journal 的重建",
            self.scn.name
        );
    }
}
