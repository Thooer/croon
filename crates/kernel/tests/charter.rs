//! 实例化一份 Charter：Collective 与它的全部成员在同一次事件里诞生。
//! 参考：docs/Kernel/Definition/README.md（Charter、Seat、接线与导出）；docs/Kernel/Journal.md（记录）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use kernel::host::{CheckError, Config, HostError, Language, Read, Session};
use kernel::index::{Provider, View, kernel_spec};
use kernel::kernel::{Conn, Kernel};
use serde_json::{Value as Json, json};

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("croon-charter-{tag}-{}", std::process::id()));
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

/// 只认 `language: "pass"`，不检查 body，也不开会话：这个测试只看诞生。
struct Pass;
impl Language for Pass {
    fn id(&self) -> &str {
        "pass"
    }
    fn check(&self, definition: &Json, _: &dyn Read) -> Result<(), CheckError> {
        if definition["language"] == "pass" {
            Ok(())
        } else {
            Err("只认 pass".into())
        }
    }
    fn open(
        &self,
        _: &str,
        _: &Json,
        _: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        Err(HostError("不在这个测试里跑".into()))
    }
}

fn config() -> Config {
    Config {
        languages: vec![Box::new(Pass)],
        ports: BTreeMap::new(),
        budgets: croon_port::Budgets {
            nodes: 10_000,
            read_bytes: 10_000_000,
            write_bytes: 10_000_000,
        },
        clock: Box::new(|| "t".into()),
        fault: None,
    }
}

fn bundle(k: &Kernel, value: &Json, deps: &[String]) -> String {
    let content = k.import(value).unwrap();
    k.import_bundle(content.as_str(), deps).unwrap()
}

fn iface(provides: Json, requires: Json) -> Json {
    json!({
        "description": "",
        "signature": {"in": {}, "out": {}},
        "provides": provides,
        "requires": requires
    })
}

fn any_port() -> Json {
    json!({"description": "收任何东西", "in": {}, "attach": {"additionalProperties": false}})
}

fn root(k: &mut Kernel, deps: &[String]) -> (String, Conn) {
    let requires = json!({
        "construct": kernel_spec("construct").0,
        "instantiate": kernel_spec("instantiate").0,
        "look": kernel_spec("look").0,
        "run": any_port()
    });
    let def = bundle(
        k,
        &json!({"evaluator": "external", "interface": iface(json!({"run": any_port()}), requires)}),
        &[],
    );
    let sources = json!({
        "construct": {"kernel": "construct", "spec": kernel_spec("construct").0},
        "instantiate": {"kernel": "instantiate", "spec": kernel_spec("instantiate").0}
    });
    let deps: Vec<String> = {
        let mut d: Vec<String> = deps.to_vec();
        d.sort();
        d.dedup();
        d
    };
    let r = k
        .create_root(
            &def,
            &json!({"value": null, "dependencies": deps}),
            &json!({"look": "look", "run": "port/run"}),
            &json!({"sources": sources}),
        )
        .unwrap();
    let c = k.connect(&r).unwrap();
    (r, c)
}

fn send(k: &mut Kernel, c: &Conn, key: &str, port: &str, value: Json, deps: &[String]) -> Json {
    k.submit(
        c,
        &json!({"key": key, "send": {
            "to": {"port": port},
            "delivery": {"value": value, "dependencies": deps},
            "attach": {}
        }}),
    )
    .unwrap()
    .unwrap()
}

/// 两个 Seat：门的 Required port 接 Collective 的，审计者接门的 Provided port。
/// 一次实例化，Collective 与两个成员在同一次事件里诞生，各自的诞生按 Charter 接线。
#[test]
fn a_charter_is_instantiated_with_its_members_in_one_event() {
    let dir = Dir::new("one-event");
    let mut k = Kernel::open(&dir.0, config()).unwrap();

    let gate = json!({
        "language": "pass",
        "interface": iface(json!({"pass": any_port()}), json!({"up": any_port()})),
        "body": null
    });
    let audit = json!({
        "language": "pass",
        "interface": iface(json!({}), json!({"see": any_port()})),
        "body": null
    });
    let gate_id = bundle(&k, &gate, &[]);
    let audit_id = bundle(&k, &audit, &[]);
    let charter = json!({
        "interface": iface(json!({"pass": any_port()}), json!({"run": any_port()})),
        "seats": {
            "gate": {"interface": iface(json!({"pass": any_port()}), json!({"up": any_port()}))},
            "audit": {"interface": iface(json!({}), json!({"see": any_port()}))}
        },
        "wire": {
            "gate": {"up": {"outer": "run"}},
            "audit": {"see": {"seat": "gate", "port": "pass"}}
        },
        "export": {"signature": "gate", "provides": {"pass": {"seat": "gate", "port": "pass"}}}
    });
    let charter_id = bundle(&k, &charter, &[]);
    let (r, c) = root(
        &mut k,
        &[gate_id.clone(), audit_id.clone(), charter_id.clone()],
    );
    for id in [&gate_id, &audit_id] {
        let out = send(
            &mut k,
            &c,
            id,
            "construct",
            json!({"definition": id}),
            &[id.clone()],
        );
        assert_eq!(
            out["outcome"]["ok"], true,
            "成员的 Definition 要能构造：{out}"
        );
    }
    let out = send(
        &mut k,
        &c,
        "charter",
        "construct",
        json!({"definition": charter_id}),
        &[charter_id.clone()],
    );
    assert_eq!(out["outcome"]["ok"], true, "Charter 要能构造：{out}");

    // Signature 所在的 Seat 不另给输入：给了是 type。
    let mut deps0 = vec![charter_id.clone(), gate_id.clone(), audit_id.clone()];
    deps0.sort();
    let run0 = k.index().journal(&r).unwrap().birth()["wire"]["run"]
        .as_str()
        .unwrap()
        .to_owned();
    let twice = json!({
        "definition": charter_id,
        "input": {"value": {"task": "t"}, "dependencies": []},
        "seats": {
            "gate": {"definition": gate_id, "input": {"value": null, "dependencies": []}},
            "audit": {"definition": audit_id}
        },
        "self": {}
    });
    let refused = k
        .submit(
            &c,
            &json!({"key": "twice", "send": {
                "to": {"port": "instantiate"},
                "delivery": {"value": twice, "dependencies": deps0},
                "attach": {"run": {"handle": run0}}
            }}),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        refused["outcome"]["error"]["kind"], "type",
        "Signature 所在的 Seat 另给输入要被拒：{refused}"
    );

    let spawn = json!({
        "definition": charter_id,
        "input": {"value": {"task": "t"}, "dependencies": []},
        "seats": {
            "gate": {"definition": gate_id},
            "audit": {"definition": audit_id, "input": {"value": null, "dependencies": []}}
        },
        "self": {}
    });
    let mut deps = vec![charter_id.clone(), gate_id.clone(), audit_id.clone()];
    deps.sort();
    deps.dedup();
    let run = k.index().journal(&r).unwrap().birth()["wire"]["run"]
        .as_str()
        .unwrap()
        .to_owned();
    let out = k
        .submit(
            &c,
            &json!({"key": "spawn", "send": {
                "to": {"port": "instantiate"},
                "delivery": {"value": spawn, "dependencies": deps},
                "attach": {"run": {"handle": run}}
            }}),
        )
        .unwrap()
        .unwrap();
    assert_eq!(out["outcome"]["ok"], true, "Charter 要能实例化：{out}");
    for name in ["look", "notify", "correct"] {
        assert!(
            out["attach"][name]["handle"].is_string(),
            "Collective 的 Control 要给出：{name}"
        );
    }

    // 一次事件造出三本 Journal：Collective 与两个成员。
    let collective = k
        .index()
        .journals
        .iter()
        .find(|(_, j)| j.birth().get("staffing").is_some())
        .map(|(i, _)| i.clone())
        .expect("Collective 有一本 Journal");
    let members: Vec<String> = k
        .index()
        .journals
        .iter()
        .filter(|(_, j)| j.birth().get("collective").is_some())
        .map(|(i, _)| i.clone())
        .collect();
    assert_eq!(members.len(), 2, "两个成员");

    let birth = k.index().journal(&collective).unwrap().birth().clone();
    assert_eq!(birth["definition"], json!(charter_id));
    assert_eq!(birth["staffing"]["gate"]["definition"], json!(gate_id));
    assert_eq!(birth["staffing"]["audit"]["definition"], json!(audit_id));

    // 按 Seat 找成员，不靠 Journal 的次序：那由 ID 的哈希决定，不是语义。
    let view = View {
        ix: k.index(),
        store: k.store(),
    };
    let gate_member = view.member_of(&collective, "gate").expect("找得到门");
    let audit_member = view.member_of(&collective, "audit").expect("找得到审计者");
    for (name, member) in [("gate", &gate_member), ("audit", &audit_member)] {
        let b = k.index().journal(member).unwrap().birth().clone();
        assert_eq!(b["collective"], json!(collective));
        assert_eq!(b["seat"], json!(name));
    }

    // 导出口：外面拿到的那一份落到门的 pass，链上两节，Seat 的在前、Collective 的在后。
    let exported = out["attach"]["port/pass"]["handle"].as_str().expect("导出口给出").to_owned();
    assert_eq!(
        view.provider(&exported),
        Some(Provider::Provided { instance: gate_member.clone(), port: "pass".to_owned() }),
        "导出口指向占 gate 的成员"
    );
    assert_eq!(view.chain(&exported).expect("导出口有链").len(), 2, "Seat 一节加 Collective 一节");

    // Collective 的输入只交给 Signature 所在的门；审计者用自己那份。
    let gate_in = k.index().journal(&gate_member).unwrap().birth()["input"].clone();
    let audit_in = k.index().journal(&audit_member).unwrap().birth()["input"].clone();
    assert_eq!(gate_in, birth["input"], "门拿到的是 Collective 的输入");
    assert_ne!(audit_in, birth["input"], "审计者不拿 Collective 的输入");

    // 审计者的 see 接的是门的 pass：链第一节是门的 Provided port，第二节是它自己的 Required port。
    let audit_birth = k.index().journal(&audit_member).unwrap().birth().clone();
    let handle = audit_birth["wire"]["see"].as_str().unwrap().to_owned();
    // 一次事件生出多本 Journal，重建时的顺序不能改变结果：事件头固定是 Collective，
    // 成员靠 (Collective, Seat) 找得到。
    let rebuilt =
        kernel::index::Index::rebuild(k.journal().all().unwrap().into_iter().rev().collect());
    assert_eq!(&rebuilt, k.index(), "以相反的次序重建，索引相同");
    assert_eq!(
        rebuilt.born_by.values().next().map(String::as_str),
        Some(collective.as_str()),
        "事件头是 Collective，不是它先写下的那个成员"
    );
    let again = View {
        ix: &rebuilt,
        store: k.store(),
    };
    assert_eq!(
        again.member_of(&collective, "gate"),
        Some(gate_member.clone())
    );

    let chain = view.chain(&handle).expect("成员接上的那一份有链");
    assert_eq!(chain.len(), 2, "来源一节加自己一节");
    match view.provider(&handle) {
        Some(Provider::Provided { instance, port }) => {
            assert_eq!(port, "pass");
            assert_eq!(
                instance,
                gate_member.clone(),
                "指向占 gate 的那个成员"
            );
        }
        other => panic!("应当指向门，得到 {other:?}"),
    }

    // 经导出口交出：view 用完之后再可变借用 k。
    let sent = k
        .submit(
            &c,
            &json!({"key": "knock", "send": {
                "to": {"handle": exported},
                "delivery": {"value": null, "dependencies": []},
                "attach": {}
            }}),
        )
        .unwrap()
        .unwrap();
    assert_eq!(sent["outcome"]["ok"], true, "经导出口交出：{sent}");
    let last = k.index().journal(&gate_member).unwrap().records.last().unwrap().clone();
    assert_eq!(last["what"]["arrival"]["port"], "pass", "到达落在门那里：{last}");
    assert_eq!(last["what"]["arrival"]["sender"], json!(r), "交出的是 root");
}
