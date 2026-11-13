//! 场景的预期自洽：读文档里全部场景的 block，Kernel 的与 Language 的，按文档里的规则查。
//! 参考：docs/Kernel/Scenarios/README.md（格式）；docs/Kernel/Journal.md（记录、四条不变量）。
//! 规则只在文档里：形状取自 instance.record、definition.schema、definition.eval 与 reference.kernel。

use std::collections::{BTreeMap, BTreeSet};

use croon_eval::schema::{json_eq, validate};
use scenarios::blocks;
use scenarios::scenario::{self, values};
use serde_json::{Value as Json, json};

/// 一个场景：名字与展开过的 block。
fn scenarios() -> Vec<(&'static str, Json)> {
    let found = scenario::all();
    assert!(
        found.len() >= 7,
        "文档里应当至少有首刀的六个场景与审批，实际 {}",
        found.len()
    );
    found
}

/// 一本 Journal：Instance → 按序号排的记录，补上 seq 与 at。
fn journals(scenario: &Json) -> BTreeMap<String, Vec<Json>> {
    let mut out = scenario::journals(scenario);
    for records in out.values_mut() {
        for record in records {
            record["at"] = json!("t");
        }
    }
    out
}

fn kind(record: &Json) -> &str {
    record["what"]
        .as_object()
        .and_then(|m| m.keys().next())
        .map_or("", String::as_str)
}

fn body(record: &Json) -> &Json {
    &record["what"][kind(record)]
}

fn must(report: croon_eval::schema::Report, at: &str) {
    if let Err(v) = report {
        panic!("{at}：{v:?}");
    }
}

#[test]
fn every_scenario_record_fits_instance_record_and_every_definition_fits_the_schema() {
    let all = scenarios();
    let values = values(&all);
    let record = blocks::json("instance.record");
    let definition = blocks::json("definition.schema");
    for (name, scenario) in &all {
        for (instance, records) in journals(scenario) {
            for r in &records {
                must(
                    validate(&record, r),
                    &format!("{name} {instance} 序号 {}", r["seq"]),
                );
            }
            assert_eq!(
                kind(&records[0]),
                "birth",
                "{name} {instance}：第一条是诞生"
            );
            let d = body(&records[0])["definition"].as_str().unwrap();
            let content = values
                .get(d)
                .unwrap_or_else(|| panic!("{name} {instance}：Definition {d} 要写在 values 里"));
            must(
                validate(&definition, &content["$bundle"]),
                &format!("{name} 的 Definition {d}"),
            );
        }
    }
    for (id, content) in &values {
        if let Some(d) = content
            .get("$bundle")
            .filter(|d| d.get("interface").is_some())
        {
            must(
                validate(&definition, d),
                &format!("完整对象 {id} 的 Definition"),
            );
        }
    }
}

#[test]
fn keys_are_unique_and_a_journal_ends_with_its_end() {
    for (name, scenario) in scenarios() {
        for (instance, records) in journals(&scenario) {
            let mut keys = BTreeSet::new();
            for r in &records {
                if let Some(key) = body(r).get("key").and_then(Json::as_str) {
                    assert!(
                        keys.insert(key.to_owned()),
                        "{name} {instance}：键 {key} 有两条记录"
                    );
                }
            }
            if let Some(end) = records.iter().position(|r| kind(r) == "end") {
                assert_eq!(
                    end + 1,
                    records.len(),
                    "{name} {instance}：已结束的 Journal 不再记"
                );
            }
        }
    }
}

#[test]
fn events_line_up() {
    for (name, scenario) in scenarios() {
        let journals = journals(&scenario);
        // 事件 → 造成它的那一步：(Instance, 序号)。
        let mut causes: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for (instance, records) in &journals {
            for (seq, r) in records.iter().enumerate() {
                if r["by"] == "self"
                    && matches!(kind(r), "send" | "end")
                    && let Some(e) = r["event"].as_str()
                {
                    let again = causes.insert(e.to_owned(), (instance.clone(), seq));
                    assert!(again.is_none(), "{name}：事件 {e} 有两条造成它的记录");
                }
            }
        }
        let mut caused: BTreeSet<String> = BTreeSet::new();
        for (instance, records) in &journals {
            for (seq, r) in records.iter().enumerate() {
                let at = format!("{name} {instance} 序号 {seq}");
                let by = r["by"].as_str().unwrap();
                let Some(e) = r["event"].as_str() else {
                    assert!(
                        by == "self" || (by == "outside" && kind(r) == "birth"),
                        "{at}：别人造成的记录要带事件"
                    );
                    continue;
                };
                let (cause, cause_seq) = causes
                    .get(e)
                    .unwrap_or_else(|| panic!("{at}：事件 {e} 没有造成它的记录"));
                if (cause, *cause_seq) == (instance, seq) {
                    continue;
                }
                caused.insert(e.to_owned());
                if cause == instance {
                    assert_eq!(
                        by, "self",
                        "{at}：自己的一步在自己这本里造成的记录 by 是 self"
                    );
                    assert!(seq > *cause_seq, "{at}：造成的记录在造成它的那一步之后");
                } else {
                    assert_eq!(by, cause, "{at}：by 是造成它的 Instance");
                }
                let invalid_before = records[..seq]
                    .iter()
                    .any(|p| p["event"] == r["event"] && kind(p) == "invalid");
                assert!(
                    !(kind(r) == "arrival" && invalid_before),
                    "{at}：同一事件先写到达，再写失效"
                );
            }
        }
        for e in causes.keys() {
            assert!(
                caused.contains(e),
                "{name}：事件 {e} 没有造成任何记录，那一步不该带事件"
            );
        }
    }
}

#[test]
fn every_receive_takes_an_earlier_arrival_that_fits_its_filter_once() {
    for (name, scenario) in scenarios() {
        for (instance, records) in journals(&scenario) {
            let mut taken = BTreeSet::new();
            for (seq, r) in records
                .iter()
                .enumerate()
                .filter(|(_, r)| kind(r) == "receive")
            {
                let receive = body(r);
                let Some(n) = receive["result"].as_u64() else {
                    continue;
                };
                let at = format!("{name} {instance} 序号 {seq}");
                let n = usize::try_from(n).unwrap();
                assert!(
                    n < seq && kind(&records[n]) == "arrival",
                    "{at}：取到的 {n} 要是之前的一项到达"
                );
                assert!(taken.insert(n), "{at}：到达 {n} 被取了两次");
                let arrival = body(&records[n]);
                let filter = &receive["filter"];
                for (field, value) in [
                    ("ports", &arrival["port"]),
                    ("via", &arrival["via"]),
                    ("sender", &arrival["sender"]),
                ] {
                    if let Some(allowed) = filter.get(field).and_then(Json::as_array) {
                        assert!(
                            allowed.contains(value),
                            "{at}：到达 {n} 的 {field} 不合 filter"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn values_and_results_fit_the_port_specs_they_pass() {
    let all = scenarios();
    let values = values(&all);
    let kernel = blocks::json("reference.kernel");
    let kernel = kernel.as_object().unwrap();
    let spec = |v: &Json| v.is_object().then(|| v.clone());
    let mut checked = 0;
    for (name, scenario) in &all {
        let journals = journals(scenario);
        // Handle → 链上已知的几节：接上它的 Required port 写的，与所指 Port 的 Port spec。
        let mut specs: BTreeMap<String, Vec<Json>> = BTreeMap::new();
        let mut note = |handle: &Json, s: Option<Json>| {
            if let Some(s) = s {
                specs
                    .entry(handle.as_str().unwrap().to_owned())
                    .or_default()
                    .push(s);
            }
        };
        for (instance, records) in &journals {
            let birth = body(&records[0]);
            let iface = &values[birth["definition"].as_str().unwrap()]["$bundle"]["interface"];
            for (port, handle) in birth["wire"].as_object().unwrap() {
                let declared = iface["requires"][port].clone();
                if records[0]["by"] == "outside" {
                    // root entry：self 里的接到自己，其余的要等于 reference.kernel 里的一项。
                    match birth.get("self").and_then(|s| s.get(port)) {
                        Some(own) => {
                            let provided = own.as_str().unwrap().strip_prefix("port/").unwrap();
                            assert!(
                                json_eq(&declared, &iface["provides"][provided]),
                                "{name} {instance}：{port} 接到自己的 {provided}，Port spec 要相同"
                            );
                        }
                        None if birth.get("binding").is_some() => {
                            let source = &birth["binding"]["sources"][port];
                            let actual = source["spec"].clone();
                            assert!(kernel::index::same_spec(&actual, "", &declared, ""));
                        }
                        None => assert!(
                            kernel.values().any(|k| json_eq(k, &declared)),
                            "{name} {instance}：{port} 不是 Kernel 的 Port"
                        ),
                    }
                }
                note(handle, Some(declared));
            }
            for r in records {
                let b = body(r);
                let incoming = match kind(r) {
                    "send" => b["result"].get("attach").cloned(),
                    "arrival" => Some(b["attach"].clone()),
                    _ => None,
                };
                for (_, i) in incoming.iter().flat_map(|m| m.as_object().unwrap()) {
                    note(&i["handle"], spec(&i["spec"]));
                }
                for (_, d) in b["result"]
                    .get("derived")
                    .and_then(Json::as_object)
                    .into_iter()
                    .flatten()
                {
                    note(&d["revoker"]["handle"], spec(&d["revoker"]["spec"]));
                }
            }
        }
        for (instance, records) in &journals {
            for r in records.iter().filter(|r| kind(r) == "send") {
                let send = body(r);
                let outcome = &send["result"]["outcome"];
                let (Some(handle), Some(true)) = (send["ref"].as_str(), outcome["ok"].as_bool())
                else {
                    continue;
                };
                let at = format!("{name} {instance} 键 {}", send["key"]);
                for s in specs.get(handle).into_iter().flatten() {
                    let raw = send["delivery"]["value"]
                        .as_str()
                        .and_then(|id| values.get(id))
                        .cloned();
                    let v = raw
                        .as_ref()
                        .and_then(|v| {
                            v.get("$alias")
                                .and_then(Json::as_str)
                                .and_then(|id| values.get(id))
                                .cloned()
                        })
                        .or(raw);
                    if let Some(v) = v {
                        must(
                            validate(&s["in"], &v),
                            &format!("{at}：交出的 Value 不合 in"),
                        );
                    }
                    if let Some(out) = s.get("out") {
                        must(
                            validate(&out["value"], &outcome["value"]),
                            &format!("{at}：结果不合 out"),
                        );
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 30, "应当查到大部分交出，实际 {checked} 次");
}

#[test]
fn kernel_port_and_session_names_use_snake_case() {
    fn name(value: &str) {
        assert!(
            value.as_bytes().first().is_some_and(u8::is_ascii_lowercase),
            "{value}"
        );
        assert!(
            value
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'),
            "{value}"
        );
    }
    fn walk(value: &Json) {
        match value {
            Json::Object(fields) => {
                for key in ["properties", "$defs"] {
                    if let Some(names) = fields.get(key).and_then(Json::as_object) {
                        for key in names.keys() {
                            name(key);
                        }
                    }
                }
                if let Some(values) = fields.get("enum").and_then(Json::as_array) {
                    for value in values.iter().filter_map(Json::as_str) {
                        name(value);
                    }
                }
                for value in fields.values() {
                    walk(value);
                }
            }
            Json::Array(values) => {
                for value in values {
                    walk(value);
                }
            }
            _ => {}
        }
    }
    walk(&blocks::json("reference.kernel"));
    walk(&blocks::json("definition.eval"));
}

#[test]
fn pending_atoms_fit_definition_eval() {
    let out = blocks::json("definition.eval")["out"].clone();
    let atom = json!({ "$ref": "#/$defs/atom", "$defs": out["$defs"] });
    for (name, scenario) in scenarios() {
        let journals = journals(&scenario);
        for (instance, atoms) in scenario
            .get("pending")
            .and_then(Json::as_object)
            .into_iter()
            .flatten()
        {
            assert!(
                journals.contains_key(instance),
                "{name}：pending 里的 {instance} 没有 Journal"
            );
            for a in atoms.as_array().unwrap() {
                must(validate(&atom, a), &format!("{name} {instance} 的 pending"));
            }
        }
        for o in scenario
            .get("order")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            for side in ["first", "then"] {
                let (i, n) = (o[side][0].as_str().unwrap(), o[side][1].as_u64().unwrap());
                let len = journals.get(i).map_or(0, Vec::len);
                assert!(
                    usize::try_from(n).unwrap() < len,
                    "{name}：order 里的 [{i}, {n}] 没有这条记录"
                );
            }
        }
    }
}
