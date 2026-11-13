//! Definition 的构造检查。
//! 参考：docs/Kernel/Definition/Shape.md（构造时检查）。
//!
//! 形状部分在 `croon-eval`；这里补齐要问 Kernel 的那几项，并保留 Kernel 自己那几份 Schema 的自检。

#[cfg(test)]
mod tests {
    use crate::blocks;
    use croon_eval::definition::check_shape;
    use croon_eval::schema::Violation;
    use serde_json::{Value as Json, json};

    fn port_min() -> Json {
        json!({"description": "d", "in": {}, "attach": {"additionalProperties": false}})
    }

    fn cognition(provides: Json, requires: Json) -> Json {
        json!({
            "language": "fake@1",
            "interface": {
                "description": "测试用",
                "signature": {"in": {"type": "object"}, "out": {"type": "string"}},
                "provides": provides,
                "requires": requires
            },
            "body": null
        })
    }

    fn violations(def: &Json) -> Vec<Violation> {
        check_shape(def).expect_err(&format!("应当被拒：{def}"))
    }

    fn registry() -> serde_json::Map<String, Json> {
        blocks::json("reference.kernel")
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn every_kernel_port_can_be_required_with_its_description_unchanged() {
        let requires: serde_json::Map<String, Json> = registry();
        check_shape(&cognition(json!({}), Json::Object(requires)))
            .unwrap_or_else(|v| panic!("{v:?}"));
    }

    #[test]
    fn a_provided_port_may_not_have_an_out() {
        let llm = registry()["pack"].clone();
        let v = violations(&cognition(json!({"chat": llm}), json!({})));
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].path, "/interface/provides/chat/out");
        assert!(
            v[0].message.contains("Provided port 没有 out"),
            "{}",
            v[0].message
        );
    }

    #[test]
    fn nested_attached_ports_may_carry_an_out() {
        // 附带过去的可以是 Kernel 的 Port 那样带 out 的。
        let mut p = port_min();
        p["attach"] = json!({"properties": {"engine": registry()["pack"].clone()}, "additionalProperties": false});
        check_shape(&cognition(json!({"p": p}), json!({}))).unwrap();
    }

    #[test]
    fn an_external_evaluator_definition_has_only_an_interface() {
        let mut iface = cognition(json!({}), json!({}))["interface"].clone();
        iface["requires"] = json!({"engine": registry()["pack"].clone()});
        check_shape(&json!({"evaluator": "external", "interface": iface})).unwrap();
        let bad = json!({"evaluator": "external", "interface": iface, "body": 1});
        violations(&bad);
    }
}
