//! Kernel scope derivation, independently of the Language consumption trace.
//! 参考：docs/Kernel/Reference.md（求值中的读取位置）。

use super::*;
use crate::host::HostError;
use crate::store::Budgets;
use crate::testutil::TempDir;

fn config() -> Config {
    Config {
        languages: vec![],
        ports: BTreeMap::new(),
        budgets: Budgets {
            nodes: 1000,
            read_bytes: 1_000_000,
            write_bytes: 1_000_000,
        },
        clock: Box::new(|| "t".into()),
        fault: None,
    }
}
fn bundle(k: &Kernel, value: Json) -> String {
    let id = k.import(&value).unwrap();
    k.import_bundle(id.as_str(), &[]).unwrap()
}

// This checks the Kernel's scope derivation directly; the Language owns the truth of its consumed trace.
#[test]
fn reader_uses_only_birth_and_named_results_and_receive_uses_its_fixed_arrival() {
    let dir = TempDir::new("consumed");
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    let ordinary = bundle(&k, json!("ordinary"));
    let later = bundle(&k, json!("future"));
    let def = bundle(
        &k,
        json!({"evaluator":"external","interface":{"description":"","signature":{"in":{},"out":{}},"provides":{},"requires":{}}}),
    );
    let i = k
        .create_root(
            &def,
            &json!({"value":null,"dependencies":[]}),
            &json!({}),
            &json!({"sources":{}}),
        )
        .unwrap();
    let c = k.connect(&i).unwrap();
    let value = k
        .import(&json!({"content":"not-a-root","dependencies":[]}))
        .unwrap();
    let arrival = raw(
        "someone",
        json!({"arrival":{"port":"in","sender":"someone","via":"route","delivery":{"value":value.as_str(),"dependencies":[ordinary]},"attach":{}}}),
    );
    let arrival_seq = k.index.journals[&i].records.len();
    k.commit(Plan {
        writes: vec![Write::Caused(i.clone(), arrival)],
        effects: vec![],
    })
    .unwrap();
    let send=k.record("self",None,json!({"send":{"key":"future","to":{"handle":"absent"},"ref":null,"delivery":{"value":value.as_str(),"dependencies":[]},"attach":{},"result":{"outcome":{"ok":true,"value":null,"dependencies":[later]},"attach":{},"derived":{},"accepted":true}}}));
    k.commit(Plan {
        writes: vec![Write::Step(i.clone(), c.generation, send)],
        effects: vec![],
    })
    .unwrap();
    let receive = k.record(
        "self",
        None,
        json!({"receive":{"key":"take","filter":{"ports":["in"]},"result":arrival_seq}}),
    );
    k.commit(Plan {
        writes: vec![Write::Step(i.clone(), c.generation, receive)],
        effects: vec![],
    })
    .unwrap();
    let reader = Reader::new(&k, &i, None);
    assert!(reader.bundle(&ordinary, &[]).unwrap().is_none());
    assert!(reader.bundle(&later, &[]).unwrap().is_none());
    assert!(
        reader
            .bundle(&ordinary, &["take".into()])
            .unwrap()
            .is_some()
    );
    assert!(reader.bundle(&later, &["take".into()]).unwrap().is_none());
    assert!(reader.bundle(&later, &["future".into()]).unwrap().is_some());
    assert!(
        reader
            .bundle(&ordinary, &["future".into()])
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        reader.bundle(&ordinary, &["invented".into()]),
        Err(HostError(_))
    ));
    assert!(reader.code_bundle(&ordinary).unwrap().is_none());
    assert!(reader.code_bundle(&def).unwrap().is_some());
    // Bytes that happen to be a descriptor stay ordinary under the consumed request Value grant.
    assert!(
        reader
            .json(value.as_str(), &["future".into()])
            .unwrap()
            .is_some()
    );
    assert!(
        reader
            .bundle(value.as_str(), &["future".into()])
            .unwrap()
            .is_none()
    );
}

#[test]
fn reader_scope_switches_share_one_operation_budget() {
    let dir = TempDir::new("read-budget");
    let mut k = Kernel::open(&dir.0, config()).unwrap();
    let def = bundle(
        &k,
        json!({"evaluator":"external","interface":{"description":"","signature":{"in":{},"out":{}},"provides":{},"requires":{}}}),
    );
    let i = k
        .create_root(
            &def,
            &json!({"value":"input","dependencies":[]}),
            &json!({}),
            &json!({"sources":{}}),
        )
        .unwrap();
    let c = k.connect(&i).unwrap();
    let receive = k.record(
        "self",
        None,
        json!({"receive":{"key":"empty","filter":{"ports":[],"timeout":0},"result":null}}),
    );
    k.commit(Plan {
        writes: vec![Write::Step(i.clone(), c.generation, receive)],
        effects: vec![],
    })
    .unwrap();
    k.config.budgets.nodes = 1;
    let reader = Reader::new(&k, &i, None);
    assert!(reader.code_bundle(&def).unwrap().is_some());
    assert!(
        reader.bundle(&def, &["empty".into()]).is_err(),
        "a new consumed scope must not reset the node budget"
    );
    k.config.budgets.nodes = 100;
    let input = k.index.journals[&i].birth()["input"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    let descriptor = k
        .store
        .get_bytes(&ValueId::parse(&def).unwrap())
        .unwrap()
        .unwrap()
        .len();
    let bytes = k
        .store
        .get_bytes(&ValueId::parse(&input).unwrap())
        .unwrap()
        .unwrap()
        .len();
    k.config.budgets.read_bytes = 2 * descriptor + bytes;
    let reader = Reader::new(&k, &i, None);
    assert!(reader.bytes(&input, &[]).unwrap().is_some());
    assert!(
        reader.bytes(&input, &["empty".into()]).is_err(),
        "scope cache misses must share consumed byte budget"
    );
}

#[test]
fn out_rejection_preserves_artifacts_and_discards_candidate_effects() {
    let dir = TempDir::new("out-artifacts");
    let k = Kernel::open(&dir.0, config()).unwrap();
    let spec = json!({"description":"","in":{},"attach":{"additionalProperties":false},"out":{"value":{"type":"integer"},"attach":{"additionalProperties":false}}});
    let node = Node {
        spec: k.import(&spec).unwrap().to_string(),
        ptr: String::new(),
        author: None,
    };
    let root = bundle(&k, json!("artifact"));
    let mut output = Outcome::ok(json!("wrong type"));
    output.outcome["dependencies"] = json!([root]);
    output
        .creates
        .push(("candidate".into(), raw("self", json!({}))));
    output.caused.push(("other".into(), raw("self", json!({}))));
    output.effects.push(Effect::Spawned("candidate".into()));
    output.attach = json!({"wrong":{"handle":"h","spec":spec}});
    output.derived = json!({"wrong":{}});
    let checked = k.checked(output, &[node]);
    assert_eq!(checked.outcome["error"]["kind"], "use");
    assert_eq!(checked.outcome["dependencies"], json!([root]));
    assert!(checked.accepted);
    assert!(checked.creates.is_empty());
    assert!(checked.caused.is_empty());
    assert!(checked.effects.is_empty());
    assert_eq!(checked.attach, json!({}));
    assert_eq!(checked.derived, json!({}));
}
