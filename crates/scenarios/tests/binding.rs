//! 同形 Port 的实际来源与读法；独立求值器只从自己收到的结果作判断。
//! 参考：docs/Kernel/Scenarios/Binding.md；docs/Kernel/Reference.md（能读的 ID）。

use kernel::host::{CheckError, HostError, Language, Read, Session};
use kernel::index::{Provider, View};
use scenarios::{
    blocks,
    drive::Setup,
    scenario::{self, Scn},
};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

struct Binding;
struct BindingSession {
    input: Json,
    results: BTreeMap<String, Json>,
    reported: BTreeSet<String>,
    body: String,
}
impl Language for Binding {
    fn id(&self) -> &str {
        "binding@1"
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
        Ok(Box::new(BindingSession {
            input: birth["input"]["value"].clone(),
            results: results.clone(),
            reported: BTreeSet::new(),
            body: birth["definition"]["body"].as_str().unwrap().into(),
        }))
    }
}
impl Session for BindingSession {
    fn resume(&mut self, new: &BTreeMap<String, Json>, _: &dyn Read) -> Result<Json, HostError> {
        self.results.extend(new.clone());
        if self.body != "binding" {
            return Ok(json!({"atoms":[]}));
        }
        for (key, port, value) in [
            ("make", "make", json!({"definition":self.input["a"]})),
            ("check", "check", json!({"definition":self.input["b"]})),
            (
                "a",
                "spawn",
                json!({"definition":self.results.get("make").map(|r| r["outcome"]["value"]["id"].clone()),"input":{"value":null,"dependencies":[]},"seats":{},"self":{}}),
            ),
            (
                "b",
                "spawn",
                json!({"definition":self.results.get("check").map(|r| r["outcome"]["value"]["id"].clone()),"input":{"value":null,"dependencies":[]},"seats":{},"self":{}}),
            ),
        ] {
            if self.results.contains_key(key) {
                continue;
            }
            let dependency = if matches!(key, "make" | "a") {
                self.input["a"].clone()
            } else {
                self.input["b"].clone()
            };
            return Ok(if self.reported.insert(key.into()) {
                json!({"atoms":[{"key":key,"send":{"to":{"port":port},"delivery":{"value":value,"dependencies":[dependency]},"attach":{}}}]})
            } else {
                json!({"atoms":[]})
            });
        }
        Ok(
            json!({"done":{"ok":true,"value":{"a":self.results["a"]["outcome"]["ok"],"b":self.results["b"]["outcome"]["error"]["kind"]},"dependencies":[]}}),
        )
    }
}
fn fixture() -> (Rc<Scn>, Setup) {
    let (all, mut values) = scenario::kernel();
    fn language(v: &mut Json) {
        match v {
            Json::Object(m) => {
                if m.get("language").is_some_and(|x| x == "fake@1") {
                    m.insert("language".into(), json!("binding@1"));
                }
                for v in m.values_mut() {
                    language(v);
                }
            }
            Json::Array(a) => {
                for v in a {
                    language(v);
                }
            }
            _ => {}
        }
    }
    for (name, value) in &mut values {
        if name.starts_with("sha256:binding") {
            language(value);
        }
    }
    let scene = all
        .into_iter()
        .find(|s| s.name == "scenario.binding")
        .unwrap();
    let mut setup = Setup::new(&values, || vec![Box::new(Binding)]);
    setup.preload = vec!["sha256:bindinga".into(), "sha256:bindingb".into()];
    (Rc::new(scene), setup)
}
#[test]
fn same_shape_keeps_distinct_sources_and_only_construct_grants_definition_reading() {
    let (scene, setup) = fixture();
    let run = setup.run(&scene, 0, None);
    assert_eq!(run.crashes, 3);
    let kernel = run.kernel.as_ref().unwrap();
    let shared = run.shared.borrow();
    let ids = &shared.bind.fwd;
    let view = View {
        ix: kernel.index(),
        store: kernel.store(),
    };
    assert_eq!(
        view.provider(&ids["h-bindmake"]),
        Some(Provider::Kernel("construct".into()))
    );
    assert_eq!(
        view.provider(&ids["h-bindchildcheck"]),
        Some(Provider::Host(ids["h-bindcheck"].clone()))
    );
    for h in [
        "h-bindmake",
        "h-bindcheck",
        "h-bindchildmake",
        "h-bindchildcheck",
    ] {
        let chain = view.chain(&ids[h]).unwrap();
        assert_eq!(chain[0].ptr, "");
        assert!(
            view.load(&chain[0].spec).is_some(),
            "source spec is readable"
        );
    }
    assert!(view.constructed(&ids["sha256:bindinga"]));
    assert!(!view.constructed(&ids["sha256:bindingb"]));
    let look = view.chain(&ids["h-bindchildjournal"]).unwrap();
    let actual = view.load(&look[0].spec).expect("Look spec is readable");
    assert!(kernel::index::same_spec(
        &actual,
        "",
        &blocks::json("reference.kernel")["look"],
        ""
    ));
}
#[test]
fn bound_sources_recover_at_every_commit_without_changing_the_observed_result() {
    let (scene, setup) = fixture();
    let commits = setup.run(&scene, 0, None).faults.borrow().commits;
    for at in 1..=commits {
        for after in [false, true] {
            setup.run(&scene, 0, Some((at, after)));
        }
    }
}
