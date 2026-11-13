//! Bundle delivery, complete Definition identity, and durable provider recovery.
//! 参考：docs/Kernel/Reference.md（能读的 ID）；docs/Kernel/Instance.md（执行）。

use croon_port::{Budgets, Content};
use kernel::{
    host::{
        Call, Channel, CheckError, Config, Execution, Fault, HostDone, HostError, Language, Port,
        Read, Session,
    },
    index::kernel_spec,
    kernel::{Conn, Kernel},
};
use serde_json::{Value as Json, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
};

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("croon-bundle-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn budgets() -> Budgets {
    Budgets {
        nodes: 10_000,
        read_bytes: 10_000_000,
        write_bytes: 10_000_000,
    }
}
fn config() -> Config {
    Config {
        languages: vec![],
        ports: BTreeMap::new(),
        budgets: budgets(),
        clock: Box::new(|| "t".into()),
        fault: None,
    }
}
fn iface(requires: Json) -> Json {
    json!({"description":"","signature":{"in":{},"out":{}},"provides":{},"requires":requires})
}
fn bundle(k: &Kernel, value: &Json, deps: &[String]) -> String {
    let content = k.import(value).unwrap();
    k.import_bundle(content.as_str(), deps).unwrap()
}
fn root(k: &mut Kernel, deps: &[String], host: bool) -> (String, Conn) {
    let mut requires = json!({"pack":kernel_spec("pack").0,"construct":kernel_spec("construct").0,"instantiate":kernel_spec("instantiate").0,"look":kernel_spec("look").0});
    let mut sources = json!({"pack":{"kernel":"pack","spec":kernel_spec("pack").0},"construct":{"kernel":"construct","spec":kernel_spec("construct").0},"instantiate":{"kernel":"instantiate","spec":kernel_spec("instantiate").0}});
    if host {
        requires["host"] = host_spec();
        sources["host"] = json!({"host":"probe","spec":host_spec()});
    }
    let def = bundle(
        k,
        &json!({"evaluator":"external","interface":iface(requires)}),
        &[],
    );
    let deps: Vec<_> = deps
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let r = k
        .create_root(
            &def,
            &json!({"value":null,"dependencies":deps}),
            &json!({"look":"look"}),
            &json!({"sources":sources}),
        )
        .unwrap();
    let c = k.connect(&r).unwrap();
    (r, c)
}
fn atom(key: &str, port: &str, value: Json, deps: &[String]) -> Json {
    json!({"key":key,"send":{"to":{"port":port},"delivery":{"value":value,"dependencies":deps},"attach":{}}})
}
fn send(k: &mut Kernel, c: &Conn, key: &str, port: &str, value: Json, deps: &[String]) -> Json {
    k.submit(c, &atom(key, port, value, deps)).unwrap().unwrap()
}

#[test]
fn rejected_dependencies_and_bundle_shaped_bytes_do_not_grant_roots() {
    let dir = Dir::new("qualification");
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    let secret = bundle(&k, &json!("secret"), &[]);
    let disguised=k.import(&json!({"content":k.store().bundle(&secret).unwrap().unwrap().content,"dependencies":[]})).unwrap();
    assert_eq!(disguised.as_str(), secret);
    let (r, c) = root(&mut k, &[], false);
    let rejected = send(
        &mut k,
        &c,
        "bad",
        "pack",
        json!({"content":{"json":null},"dependencies":[secret]}),
        &[secret.clone()],
    );
    assert_eq!(rejected["accepted"], false);
    assert_eq!(rejected["outcome"]["dependencies"], json!([]));
    // The rejected request's value is ordinary acquired content, and can be wrapped as new content.
    let request = k.index().journals[&r].records[1]["what"]["send"]["delivery"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    let denied = send(
        &mut k,
        &c,
        "again",
        "pack",
        json!({"content":{"id":request},"dependencies":[secret]}),
        &[],
    );
    assert_eq!(denied["accepted"], false);
    let look = send(
        &mut k,
        &c,
        "lookbad",
        "look",
        json!({"op":"read","from":1,"limit":1}),
        &[],
    );
    assert_eq!(look["outcome"]["dependencies"], json!([]));
    let okay = send(
        &mut k,
        &c,
        "plain",
        "pack",
        json!({"content":{"json":{"content":secret,"dependencies":[]}},"dependencies":[]}),
        &[],
    );
    let plain = okay["outcome"]["value"]["id"].as_str().unwrap().to_owned();
    let denied = send(
        &mut k,
        &c,
        "unmask",
        "pack",
        json!({"content":{"json":null},"dependencies":[secret]}),
        &[plain],
    );
    assert_eq!(denied["accepted"], false);
    let rebuilt =
        kernel::index::Index::rebuild(k.journal().all().unwrap().into_iter().rev().collect());
    assert_eq!(&rebuilt, k.index());
}

struct Probe {
    checks: Rc<Cell<usize>>,
    extra: String,
}
impl Language for Probe {
    fn id(&self) -> &str {
        "probe"
    }
    fn check(&self, d: &Json, read: &dyn Read) -> Result<(), CheckError> {
        self.checks.set(self.checks.get() + 1);
        let source = d["body"]["source"].as_str().unwrap();
        if read.code_bundle(source)?.is_none() {
            return Err("missing fixed source".into());
        }
        assert!(read.code_json(&self.extra)?.is_none());
        Ok(())
    }
    fn open(
        &self,
        _: &str,
        _: &Json,
        _: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        Err(HostError("not run by this test".into()))
    }
}
#[test]
fn construction_uses_complete_closure_and_only_real_kernel_success_is_reusable() {
    let dir = Dir::new("construct");
    let k = Kernel::open(&dir.0, config()).unwrap();
    let source = bundle(&k, &json!("source"), &[]);
    let extra = bundle(&k, &json!("extra"), &[]);
    let def = json!({"language":"probe","interface":iface(json!({})),"body":{"source":source}});
    let missing = bundle(&k, &def, &[]);
    let complete = bundle(&k, &def, &[source]);
    drop(k);
    let checks = Rc::new(Cell::new(0));
    let mut cfg = config();
    cfg.languages.push(Box::new(Probe {
        checks: checks.clone(),
        extra: extra.clone(),
    }));
    let mut k = Kernel::open(&dir.0, cfg).unwrap();
    let (_r, c) = root(&mut k, &[missing.clone(), complete.clone(), extra], false);
    let bad = send(
        &mut k,
        &c,
        "missing",
        "construct",
        json!({"definition":missing}),
        &[missing.clone()],
    );
    assert_eq!(bad["outcome"]["ok"], false);
    let okay = send(
        &mut k,
        &c,
        "build",
        "construct",
        json!({"definition":complete}),
        &[complete.clone()],
    );
    assert_eq!(okay["outcome"]["ok"], true);
    let before = checks.get();
    let again = send(
        &mut k,
        &c,
        "reuse",
        "construct",
        json!({"definition":complete}),
        &[complete.clone()],
    );
    assert_eq!(again["outcome"]["ok"], true);
    assert_eq!(checks.get(), before);
    let spawned = send(
        &mut k,
        &c,
        "spawn",
        "instantiate",
        json!({"definition":complete,"input":{"value":null,"dependencies":[]},"self":{},"seats":{}}),
        &[complete],
    );
    assert_eq!(spawned["outcome"]["ok"], true);
    assert!(spawned["attach"]["look"]["spec"].is_object());
}

fn host_spec() -> Json {
    json!({"description":"","in":{},"attach":{"additionalProperties":false},"out":{"value":{},"attach":{"additionalProperties":false}}})
}
struct Provider {
    ready: Rc<Cell<bool>>,
    starts: Rc<RefCell<Vec<String>>>,
    retires: Rc<RefCell<Vec<String>>>,
    fail_start: bool,
}
struct Running {
    ready: Rc<Cell<bool>>,
    content: Rc<Content>,
}
impl Port for Provider {
    fn id(&self) -> &str {
        "probe"
    }
    fn channel(&self) -> Channel {
        Channel::Isolated
    }
    fn start(&self, c: &Call) -> Result<Box<dyn Execution>, HostError> {
        self.starts.borrow_mut().push(c.attempt.into());
        if self.fail_start {
            return Err(HostError("after physical start".into()));
        }
        Ok(Box::new(Running {
            ready: self.ready.clone(),
            content: c.content.clone(),
        }))
    }
    fn retire(&self, id: &str) -> Result<(), HostError> {
        self.retires.borrow_mut().push(id.into());
        Ok(())
    }
}
impl Execution for Running {
    fn collect(&mut self) -> Result<Option<HostDone>, HostError> {
        if !self.ready.get() {
            return Ok(None);
        }
        let value = self.content.put_json(&json!("artifact"))?;
        let root = self.content.pack(&value, &[])?;
        Ok(Some(HostDone {
            outcome: json!({"ok":false,"error":{"kind":"tool","message":"failed with output","data":null},"dependencies":[root]}),
        }))
    }
}
#[test]
fn durable_attempt_recovers_start_failure_and_committed_result_without_reexecution() {
    for after_commit in [false, true] {
        let dir = Dir::new(if after_commit { "committed" } else { "start" });
        let mut k = Kernel::open(&dir.0, config()).unwrap();
        let (r, c) = root(&mut k, &[], true);
        let source = k.index().journals[&r].wire["host"].clone();
        let starts = Rc::new(RefCell::new(vec![]));
        let retires = Rc::new(RefCell::new(vec![]));
        let ready = Rc::new(Cell::new(true));
        let port = Rc::new(Provider {
            ready: ready.clone(),
            starts: starts.clone(),
            retires: retires.clone(),
            fail_start: !after_commit,
        });
        k.bind_host(&source, port).unwrap();
        let request = atom("run", "host", json!(null), &[]);
        if after_commit {
            drop(k);
            let mut cfg = config();
            cfg.ports.insert(
                source.clone(),
                Rc::new(Provider {
                    ready: ready.clone(),
                    starts: starts.clone(),
                    retires: retires.clone(),
                    fail_start: false,
                }),
            );
            cfg.fault = Some(Box::new(
                |f| matches!(f,Fault::AfterCommit{records} if records.iter().any(|(_,k)|k.as_deref()==Some("run"))),
            ));
            k = Kernel::open(&dir.0, cfg).unwrap();
            let c = k.connect(&r).unwrap();
            assert!(k.submit(&c, &request).is_err());
        } else {
            assert!(k.submit(&c, &request).is_err());
        }
        assert_eq!(k.journal().attempts().unwrap().len(), 1);
        let old = starts.borrow()[0].clone();
        drop(k);
        let mut cfg = config();
        cfg.ports.insert(
            source.clone(),
            Rc::new(Provider {
                ready,
                starts: starts.clone(),
                retires: retires.clone(),
                fail_start: false,
            }),
        );
        let mut k = Kernel::open(&dir.0, cfg).unwrap();
        assert!(retires.borrow().contains(&old));
        k.run().unwrap();
        let c = k.connect(&r).unwrap();
        let result = k.submit(&c, &request).unwrap().unwrap();
        assert_eq!(result["accepted"], true);
        assert_eq!(result["outcome"]["ok"], false);
        assert_eq!(
            result["outcome"]["dependencies"].as_array().unwrap().len(),
            1
        );
        assert_eq!(starts.borrow().len(), if after_commit { 1 } else { 2 });
        assert!(k.journal().attempts().unwrap().is_empty());
    }
}

#[test]
fn missing_original_provider_blocks_reopen_until_its_attempt_is_retired() {
    let dir = Dir::new("missing-provider");
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    let (r, c) = root(&mut k, &[], true);
    let source = k.index().journals[&r].wire["host"].clone();
    let starts = Rc::new(RefCell::new(vec![]));
    let retires = Rc::new(RefCell::new(vec![]));
    let ready = Rc::new(Cell::new(false));
    k.bind_host(
        &source,
        Rc::new(Provider {
            ready: ready.clone(),
            starts: starts.clone(),
            retires: retires.clone(),
            fail_start: false,
        }),
    )
    .unwrap();
    assert!(
        k.submit(&c, &atom("run", "host", Json::Null, &[]))
            .unwrap()
            .is_none()
    );
    drop(k);
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    assert!(
        !k.stopped().contains_key(&r),
        "没有来源装配时不提前判定恢复结果"
    );
    assert!(k.connect(&r).is_err());
    ready.set(true);
    k.bind_host(
        &source,
        Rc::new(Provider {
            ready,
            starts: starts.clone(),
            retires: retires.clone(),
            fail_start: false,
        }),
    )
    .unwrap();
    k.run().unwrap();
    let c = k.connect(&r).unwrap();
    let result = k
        .submit(&c, &atom("run", "host", Json::Null, &[]))
        .unwrap()
        .unwrap();
    assert_eq!(result["accepted"], true);
    assert_eq!(starts.borrow().len(), 2);
}

#[test]
fn content_budget_and_shape_errors_are_distinct_from_unreadable_content() {
    let dir = Dir::new("content");
    let store = Rc::new(croon_port::ContentStore::open(&dir.0).unwrap());
    let content = Content::from_inputs(store.clone(), &[], budgets()).unwrap();
    let value = content.put_bytes(b"abc").unwrap();
    let root = content.pack(&value, &[]).unwrap();
    let missing = Content::from_inputs(store.clone(), &[], budgets()).unwrap();
    assert!(missing.bytes(&value).unwrap().is_none());
    assert!(missing.bundle(&root).unwrap().is_none());
    let zero = Budgets {
        nodes: 0,
        ..budgets()
    };
    assert!(Content::from_inputs(store.clone(), &[root.clone()], zero).is_err());
    let readable = Content::from_inputs(store.clone(), &[root.clone()], budgets()).unwrap();
    assert_eq!(readable.bytes(&value).unwrap().unwrap(), b"abc");
    let bundle = readable.bundle(&root).unwrap().unwrap();
    let wrapped = readable.pack(&root, &[]).unwrap();
    assert_ne!(root, wrapped);
    assert_eq!(bundle.content, value);
    let hex = &value[7..];
    std::fs::write(dir.0.join(&hex[..2]).join(&hex[2..]), b"damaged").unwrap();
    assert!(readable.bytes(&value).is_err());
}

struct Producer;
struct ProduceSession {
    root: Option<String>,
}
impl Language for Producer {
    fn id(&self) -> &str {
        "producer"
    }
    fn check(&self, _: &Json, _: &dyn Read) -> Result<(), CheckError> {
        Ok(())
    }
    fn open(
        &self,
        _: &str,
        _: &Json,
        _: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        Ok(Box::new(ProduceSession { root: None }))
    }
}
impl Session for ProduceSession {
    fn resume(
        &mut self,
        results: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Json, HostError> {
        if let Some(result) = results.get("pack") {
            self.root = result["outcome"]["value"]["id"].as_str().map(str::to_owned);
        }
        Ok(match &self.root {
            None => {
                json!({"atoms":[atom("pack","pack",json!({"content":{"json":"produced"},"dependencies":[]}),&[])]})
            }
            Some(root) => json!({"done":{"ok":true,"value":root,"dependencies":[root]}}),
        })
    }
}
#[test]
fn look_delivers_only_selected_record_sets_even_after_the_target_acquires_more() {
    let dir = Dir::new("look-selection");
    let mut cfg = config();
    cfg.languages.push(Box::new(Producer));
    let mut k = Kernel::open(&dir.0, cfg).unwrap();
    let definition = bundle(
        &k,
        &json!({"language":"producer","interface":iface(json!({"pack":kernel_spec("pack").0})),"body":null}),
        &[],
    );
    let expected = bundle(&k, &json!("produced"), &[]);
    let (_r, c) = root(&mut k, &[definition.clone()], false);
    assert_eq!(
        send(
            &mut k,
            &c,
            "build",
            "construct",
            json!({"definition":definition}),
            &[definition.clone()]
        )["outcome"]["ok"],
        true
    );
    let mut spawn = atom(
        "spawn",
        "instantiate",
        json!({"definition":definition,"input":{"value":null,"dependencies":[]},"self":{},"seats":{}}),
        &[definition],
    );
    spawn["send"]["attach"] = json!({"pack":{"port":"pack"}});
    let spawned = k.submit(&c, &spawn).unwrap().unwrap();
    let look = spawned["attach"]["look"]["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    k.run().unwrap();
    let request = |key: &str, value: Json| json!({"key":key,"send":{"to":{"handle":look},"delivery":{"value":value,"dependencies":[]},"attach":{}}});
    let old = k
        .submit(&c, &request("old", json!({"op":"read","from":0,"limit":1})))
        .unwrap()
        .unwrap();
    assert!(
        !old["outcome"]["dependencies"]
            .as_array()
            .unwrap()
            .contains(&json!(expected))
    );
    let denied = send(
        &mut k,
        &c,
        "premature",
        "pack",
        json!({"content":{"json":null},"dependencies":[expected]}),
        &[expected.clone()],
    );
    assert_eq!(denied["accepted"], false);
    let inspected = k
        .submit(&c, &request("inspect", json!({"op":"inspect"})))
        .unwrap()
        .unwrap();
    assert_eq!(inspected["outcome"]["dependencies"], json!([expected]));
    assert_eq!(
        send(
            &mut k,
            &c,
            "after",
            "pack",
            json!({"content":{"json":null},"dependencies":[expected]}),
            &[expected]
        )["outcome"]["ok"],
        true
    );
}

struct Worker;
struct WorkSession {
    result: Option<Json>,
}
impl Language for Worker {
    fn id(&self) -> &str {
        "worker"
    }
    fn check(&self, _: &Json, _: &dyn Read) -> Result<(), CheckError> {
        Ok(())
    }
    fn open(
        &self,
        _: &str,
        _: &Json,
        _: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        Ok(Box::new(WorkSession { result: None }))
    }
}
impl Session for WorkSession {
    fn resume(
        &mut self,
        results: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Json, HostError> {
        if let Some(result) = results.get("run") {
            self.result = Some(result.clone());
        }
        Ok(match &self.result {
            Some(result) => json!({"done":result["outcome"]}),
            None => json!({"atoms":[atom("run","host",Json::Null,&[])]}),
        })
    }
}
#[test]
fn pause_and_rebinding_keep_the_accepted_execution_on_its_original_provider() {
    let dir = Dir::new("provider-fixed");
    let mut cfg = config();
    cfg.languages.push(Box::new(Worker));
    let mut k = Kernel::open(&dir.0, cfg).unwrap();
    let definition = bundle(
        &k,
        &json!({"language":"worker","interface":iface(json!({"host":host_spec()})),"body":null}),
        &[],
    );
    let (r, c) = root(&mut k, &[definition.clone()], true);
    let source = k.index().journals[&r].wire["host"].clone();
    let starts = Rc::new(RefCell::new(vec![]));
    let ready = Rc::new(Cell::new(false));
    k.bind_host(
        &source,
        Rc::new(Provider {
            ready: ready.clone(),
            starts: starts.clone(),
            retires: Rc::new(RefCell::new(vec![])),
            fail_start: false,
        }),
    )
    .unwrap();
    send(
        &mut k,
        &c,
        "construct",
        "construct",
        json!({"definition":definition}),
        &[definition.clone()],
    );
    let mut spawn = atom(
        "spawn",
        "instantiate",
        json!({"definition":definition,"input":{"value":null,"dependencies":[]},"self":{},"seats":{}}),
        &[definition],
    );
    spawn["send"]["attach"] = json!({"host":{"port":"host"}});
    let result = k.submit(&c, &spawn).unwrap().unwrap();
    let control = result["attach"]["correct"]["handle"].as_str().unwrap();
    k.run().unwrap();
    assert_eq!(starts.borrow().len(), 1);
    let pause = json!({"key":"pause","send":{"to":{"handle":control},"delivery":{"value":{"op":"pause"},"dependencies":[]},"attach":{}}});
    assert_eq!(
        k.submit(&c, &pause).unwrap().unwrap()["outcome"]["ok"],
        true
    );
    let replacement_starts = Rc::new(RefCell::new(vec![]));
    k.bind_host(
        &source,
        Rc::new(Provider {
            ready: Rc::new(Cell::new(true)),
            starts: replacement_starts.clone(),
            retires: Rc::new(RefCell::new(vec![])),
            fail_start: false,
        }),
    )
    .unwrap();
    ready.set(true);
    k.run().unwrap();
    assert_eq!(starts.borrow().len(), 1);
    assert!(replacement_starts.borrow().is_empty());
    let child = k
        .index()
        .born_by
        .values()
        .find(|i| i.as_str() != r)
        .unwrap();
    assert!(k.index().journals[child].keys.contains_key("run"));
    assert!(k.index().journals[child].ended.is_none());
    let mut resume = pause;
    resume["key"] = json!("resume");
    resume["send"]["delivery"]["value"] = json!({"op":"resume"});
    k.submit(&c, &resume).unwrap();
    k.run().unwrap();
    assert!(replacement_starts.borrow().is_empty());
}

#[test]
fn acceptance_write_failure_cannot_let_a_retry_start_without_a_durable_attempt() {
    let dir = Dir::new("acceptance-write-retry");
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    let (r, c) = root(&mut k, &[], true);
    let source = k.index().journals[&r].wire["host"].clone();
    let starts = Rc::new(RefCell::new(vec![]));
    k.bind_host(
        &source,
        Rc::new(Provider {
            ready: Rc::new(Cell::new(false)),
            starts: starts.clone(),
            retires: Rc::new(RefCell::new(vec![])),
            fail_start: false,
        }),
    )
    .unwrap();
    let db = rusqlite::Connection::open(dir.0.join("journal.db")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_acceptance BEFORE INSERT ON host_attempts BEGIN SELECT RAISE(FAIL, 'injected acceptance failure'); END;").unwrap();
    let request = atom("run", "host", Json::Null, &[]);
    assert!(k.submit(&c, &request).is_err());
    assert!(starts.borrow().is_empty());
    assert!(k.journal().attempts().unwrap().is_empty());
    db.execute_batch("DROP TRIGGER reject_acceptance").unwrap();
    assert!(k.submit(&c, &request).unwrap().is_none());
    assert_eq!(starts.borrow().len(), 1);
    let attempts = k.journal().attempts().unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].0, starts.borrow()[0]);
}

#[test]
fn signature_rejection_does_not_mark_instantiate_as_accepted() {
    let dir = Dir::new("signature-refused");
    let mut cfg = config();
    cfg.languages.push(Box::new(Producer));
    let mut k = Kernel::open(&dir.0, cfg).unwrap();
    let mut interface = iface(json!({}));
    interface["signature"]["in"] = json!({"type":"integer"});
    let definition = bundle(
        &k,
        &json!({"language":"producer","interface":interface,"body":null}),
        &[],
    );
    let (r, c) = root(&mut k, &[definition.clone()], false);
    send(
        &mut k,
        &c,
        "construct",
        "construct",
        json!({"definition":definition}),
        &[definition.clone()],
    );
    let result = send(
        &mut k,
        &c,
        "invalid-input",
        "instantiate",
        json!({"definition":definition,"input":{"value":"wrong","dependencies":[]},"self":{},"seats":{}}),
        &[definition],
    );
    assert_eq!(result["accepted"], false);
    assert_eq!(result["outcome"]["error"]["kind"], "type");
    assert_eq!(result["attach"], json!({}));
    assert_eq!(k.index().journals.len(), 1);
    assert!(k.index().journals.contains_key(&r));
}

#[test]
fn failed_attempt_replacement_keeps_the_old_identity_until_commit() {
    let dir = Dir::new("replace-attempt-retry");
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    let (r, c) = root(&mut k, &[], true);
    let source = k.index().journals[&r].wire["host"].clone();
    let starts = Rc::new(RefCell::new(vec![]));
    k.bind_host(
        &source,
        Rc::new(Provider {
            ready: Rc::new(Cell::new(false)),
            starts: starts.clone(),
            retires: Rc::new(RefCell::new(vec![])),
            fail_start: true,
        }),
    )
    .unwrap();
    assert!(k.submit(&c, &atom("run", "host", Json::Null, &[])).is_err());
    let old = k.journal().attempts().unwrap()[0].0.clone();
    let db = rusqlite::Connection::open(dir.0.join("journal.db")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_replacement BEFORE INSERT ON host_attempts BEGIN SELECT RAISE(FAIL, 'injected replacement failure'); END;").unwrap();
    assert!(k.run().is_err());
    assert_eq!(starts.borrow().len(), 1);
    assert_eq!(k.journal().attempts().unwrap()[0].0, old);
    db.execute_batch("DROP TRIGGER reject_replacement").unwrap();
    k.run().unwrap();
    assert_eq!(starts.borrow().len(), 2);
    let persisted = k.journal().attempts().unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].0, starts.borrow()[1]);
    assert_ne!(persisted[0].0, old);
}
