//! 校验器：只认 value.schema 的子集。
//! 参考：docs/Kernel/Value.md（Schema）。
//!
//! 实现在 `croon-eval`；本文件补齐 Kernel 自己那几份 Schema 的自检。

#[cfg(test)]
mod tests {
    use crate::blocks;
    use croon_eval::schema::{Violation, check_schema, validate};
    use serde_json::{Value as Json, json};

    fn ok(schema: Json, value: Json) {
        if let Err(v) = validate(&schema, &value) {
            panic!("应当满足，却违反了：{v:?}\nschema = {schema}\nvalue = {value}");
        }
    }

    fn fails(schema: Json, value: Json) -> Vec<Violation> {
        validate(&schema, &value)
            .expect_err(&format!("应当违反：schema = {schema}, value = {value}"))
    }

    // ---- block 里的 Schema 都合法：block 与校验器互为证据 ----

    fn registry_ports() -> Vec<(String, Json)> {
        match blocks::json("reference.kernel") {
            Json::Object(map) => map.into_iter().collect(),
            other => panic!("reference.kernel 应是对象：{other}"),
        }
    }

    #[test]
    fn every_schema_block_is_a_valid_schema() {
        for name in [
            "value.bundle",
            "value.delivery",
            "instance.record",
            "definition.outcome",
            "definition.failure",
        ] {
            check_schema(&blocks::json(name)).unwrap_or_else(|v| panic!("{name}：{v:?}"));
        }
        // definition.schema 在 croon-eval 的表里，不在 kernel 的表里。
        check_schema(&croon_eval::blocks::json("definition.schema"))
            .unwrap_or_else(|v| panic!("definition.schema：{v:?}"));
        let eval = blocks::json("definition.eval");
        for side in ["in", "out"] {
            check_schema(&eval[side]).unwrap_or_else(|v| panic!("definition.eval.{side}：{v:?}"));
        }
    }

    #[test]
    fn every_kernel_port_in_the_docs_has_valid_schemas() {
        let ports = registry_ports();
        assert!(ports.len() >= 7);
        for (name, port) in ports {
            check_schema(&port["in"])
                .unwrap_or_else(|v| panic!("reference.kernel.{name}.in：{v:?}"));
            if let Some(out) = port.get("out") {
                check_schema(&out["value"])
                    .unwrap_or_else(|v| panic!("reference.kernel.{name}.out.value：{v:?}"));
            }
        }
    }

    #[test]
    fn a_bundle_and_a_journal_record_validate_against_their_doc_schemas() {
        let bundle = blocks::json("value.bundle");
        ok(bundle.clone(), json!({"content":"id","dependencies":[]}));
        fails(bundle, json!({"content":"id"}));
        let record = blocks::json("instance.record");
        ok(
            record.clone(),
            json!({"seq": 1, "at": "2026-01-01T00:00:00Z", "by": "self", "what": {"end": {"ok": true, "value": null,"dependencies":[]}}}),
        );
        fails(
            record.clone(),
            json!({"at": "t", "by": "self", "what": {"end": 1}}),
        );
        let two = fails(
            record,
            json!({"seq": 1, "at": "t", "by": "self", "what": {"end": 1, "invalid": []}}),
        );
        assert!(two[0].message.contains("anyOf"), "{}", two[0].message);
    }

    #[test]
    fn a_refused_send_and_each_correct_op_fit_the_record_schema() {
        let record = blocks::json("instance.record");
        let h = format!("sha256:{}", "0".repeat(64));
        let at = |what: Json| json!({"seq": 1, "at": "t", "by": "self", "what": what});
        let refused = json!({"ok": false, "error": {"kind": "denied", "message": "m", "data": null},"dependencies":[]});
        // 解析不到的目标与附带也记得下：给出的目标照记，ref 为 null。
        ok(
            record.clone(),
            at(json!({"send": {
                "key": "0", "to": {"port": "engine"}, "ref": null, "delivery":{"value":h,"dependencies":[]},
                "attach": {"reply": {"to": {"port": "me"}, "ref": null}},
                "result": {"outcome": refused, "attach": {}, "derived": {},"accepted":false}
            }})),
        );
        fails(
            record.clone(),
            at(json!({"send": {
                "key": "0", "to": {"port": "engine"}, "ref": "h-1", "value": h,
                "attach": {"reply": "h-2"}, "result": {}
            }})),
        );
        // result 是 definition.eval 里交出的结果（展开自 $block），裸的结束状态不是。
        fails(
            record.clone(),
            at(json!({"send": {
                "key": "0", "to": {"port": "engine"}, "ref": null, "delivery":{"value":h,"dependencies":[]}, "attach": {}, "result": refused
            }})),
        );
        let by =
            |what: Json| json!({"seq": 1, "at": "t", "by": "i-b", "event": "e-1", "what": what});
        ok(record.clone(), by(json!({"correct": {"op": "pause"}})));
        ok(
            record.clone(),
            by(json!({"correct": {"op": "grant", "port": "engine", "ref": "h-3"}})),
        );
        // 授予的记录不写 spec：比相同时它由目标的 Definition 推得出。
        fails(
            record.clone(),
            by(json!({"correct": {"op": "grant", "port": "engine", "ref": "h-3", "spec": h}})),
        );
        ok(
            record.clone(),
            by(json!({"correct": {"op": "withdraw", "port": "engine"}})),
        );
        fails(record.clone(), by(json!({"correct": {"op": "grant"}})));
        fails(
            record,
            by(json!({"correct": {"op": "pause", "port": "engine"}})),
        );
    }

    #[test]
    fn a_failure_has_no_fields_beyond_kind_message_and_data() {
        let failure = blocks::json("definition.failure");
        ok(
            failure.clone(),
            json!({"kind": "type", "message": "m", "data": {"cause": 1}}),
        );
        fails(
            failure,
            json!({"kind": "type", "message": "m", "data": null, "cause": 1}),
        );
    }
}
