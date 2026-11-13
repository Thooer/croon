//! Definition 的构造检查：形状部分。
//! 参考：docs/Kernel/Definition/Shape.md（构造时检查）。
//!
//! 这里只查不需要 Kernel 状态的：形状满足 definition.schema；Signature 与 Port 的 in、out
//! 满足 value.schema；Provided port 没有 out；附带里的 `$ref` 指得到。Language 是否登记、
//! 依赖代码能不能读、body 是否合格，要问 Kernel 与 Language，在构造的那一步另查。

use std::sync::OnceLock;

use serde_json::Value as Json;

use crate::schema::{Report, Violation, check_schema, validate};

pub fn check_shape(definition: &Json) -> Report {
    static SCHEMA: OnceLock<Json> = OnceLock::new();
    let schema = SCHEMA.get_or_init(|| crate::blocks::json("definition.schema"));
    validate(schema, definition)?;

    let mut out = Vec::new();
    if definition.get("seats").is_some() {
        // 还没有实现的，构造被拒，不放过：放过就是一份没人检查过的 Charter。
        out.push(Violation {
            path: String::new(),
            rule: "shape",
            message: "Charter 的构造检查还没有实现，构造被拒".to_owned(),
        });
        return Err(out);
    }
    interface(&definition["interface"], "/interface", &mut out);
    if out.is_empty() { Ok(()) } else { Err(out) }
}

/// 把一份报告的路径挂到 `base` 下，并入 `out`。
fn lift(base: &str, report: Report, out: &mut Vec<Violation>) {
    if let Err(violations) = report {
        out.extend(violations.into_iter().map(|v| Violation {
            path: format!("{base}{}", v.path),
            rule: v.rule,
            message: v.message,
        }));
    }
}

fn fail(out: &mut Vec<Violation>, path: &str, message: String) {
    out.push(Violation {
        path: path.to_owned(),
        rule: "shape",
        message,
    });
}

fn escape(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

fn interface(iface: &Json, path: &str, out: &mut Vec<Violation>) {
    for side in ["in", "out"] {
        lift(
            &format!("{path}/signature/{side}"),
            check_schema(&iface["signature"][side]),
            out,
        );
    }
    for (group, allow_out) in [("provides", false), ("requires", true)] {
        if let Some(ports) = iface[group].as_object() {
            for (name, spec) in ports {
                port_spec(
                    spec,
                    allow_out,
                    &format!("{path}/{group}/{}", escape(name)),
                    out,
                );
            }
        }
    }
}

/// 一份 Port 的说明：直接写出，或者顶层带 `$defs`、指向其中一份。
/// `allow_out` 为假时（Provided port），顶层那一份不能有 out。
fn port_spec(spec: &Json, allow_out: bool, path: &str, out: &mut Vec<Violation>) {
    let target = spec.get("$ref").and_then(Json::as_str);
    if let Some(r) = target {
        if resolve(spec, r).is_none() {
            fail(
                out,
                path,
                format!("$ref {r} 在这份说明顶层的 $defs 里找不到"),
            );
        }
    } else {
        port(spec, spec, allow_out, path, out);
    }
    if let Some(defs) = spec.get("$defs").and_then(Json::as_object) {
        for (name, def) in defs {
            // 被根指向的那一份是对外的 Port，其余的是附带里嵌套的 Port，可以有 out。
            let is_target = target.is_some_and(|r| r == format!("#/$defs/{name}"));
            port(
                def,
                spec,
                allow_out || !is_target,
                &format!("{path}/$defs/{}", escape(name)),
                out,
            );
        }
    }
}

fn port(port: &Json, root: &Json, allow_out: bool, path: &str, out: &mut Vec<Violation>) {
    lift(&format!("{path}/in"), check_schema(&port["in"]), out);
    attach(&port["attach"], root, &format!("{path}/attach"), out);
    if let Some(result) = port.get("out") {
        if !allow_out {
            fail(
                out,
                &format!("{path}/out"),
                "Provided port 没有 out：要回复，就声明一个附带".to_owned(),
            );
        } else {
            lift(
                &format!("{path}/out/value"),
                check_schema(&result["value"]),
                out,
            );
            attach(&result["attach"], root, &format!("{path}/out/attach"), out);
        }
    }
}

/// 附带的声明：叶子是 `true`、一份 Port 的说明，或指向顶层 `$defs` 的 `$ref`。
fn attach(attach: &Json, root: &Json, path: &str, out: &mut Vec<Violation>) {
    if let Some(props) = attach.get("properties").and_then(Json::as_object) {
        for (name, leaf_) in props {
            leaf(
                leaf_,
                root,
                true,
                &format!("{path}/properties/{}", escape(name)),
                out,
            );
        }
    }
    if let Some(extra) = attach.get("additionalProperties") {
        leaf(
            extra,
            root,
            false,
            &format!("{path}/additionalProperties"),
            out,
        );
    }
}

fn leaf(leaf: &Json, root: &Json, in_properties: bool, path: &str, out: &mut Vec<Violation>) {
    match leaf {
        Json::Bool(false) if in_properties => {
            fail(
                out,
                path,
                "在 properties 里只能写 true、一份 Port 的说明或 $ref；不接受就不要写这个名字"
                    .to_owned(),
            );
        }
        Json::Bool(_) => {}
        Json::Object(map) if map.contains_key("$ref") => {
            let r = map["$ref"].as_str().unwrap_or_default();
            if resolve(root, r).is_none() {
                fail(
                    out,
                    path,
                    format!("$ref {r} 在这份说明顶层的 $defs 里找不到"),
                );
            }
        }
        other => port(other, root, true, path, out),
    }
}

/// `#/$defs/名字` 在这份说明顶层的位置。
fn resolve<'a>(root: &'a Json, reference: &str) -> Option<&'a Json> {
    root.get("$defs")?.get(reference.strip_prefix("#/$defs/")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn a_minimal_cognition_passes() {
        check_shape(&cognition(json!({}), json!({}))).unwrap();
    }

    #[test]
    fn a_provided_port_without_out_passes() {
        check_shape(&cognition(json!({"chat": port_min()}), json!({}))).unwrap();
    }

    #[test]
    fn invalid_signature_schemas_are_reported_with_their_path() {
        let mut def = cognition(json!({}), json!({}));
        def["interface"]["signature"]["in"] = json!({"type": "str"});
        let v = violations(&def);
        assert!(
            v[0].path.starts_with("/interface/signature/in"),
            "{}",
            v[0].path
        );
    }

    #[test]
    fn invalid_port_schemas_are_reported_with_their_path() {
        let mut bad = port_min();
        bad["in"] = json!({"pattern": "x"});
        let v = violations(&cognition(json!({"p": bad}), json!({})));
        assert!(
            v[0].path.starts_with("/interface/provides/p/in"),
            "{}",
            v[0].path
        );
    }

    #[test]
    fn a_port_spec_with_a_recursive_reply_passes() {
        let chat = json!({
            "$ref": "#/$defs/chat",
            "$defs": {"chat": {
                "description": "对话",
                "in": {"type": "string"},
                "attach": {"properties": {"reply": {"$ref": "#/$defs/chat"}}, "required": ["reply"], "additionalProperties": false}
            }}
        });
        check_shape(&cognition(json!({"chat": chat}), json!({}))).unwrap();
    }

    #[test]
    fn a_dangling_ref_in_an_attach_declaration_is_reported() {
        let chat = json!({
            "$ref": "#/$defs/chat",
            "$defs": {"chat": {
                "description": "对话", "in": {},
                "attach": {"properties": {"reply": {"$ref": "#/$defs/missing"}}, "additionalProperties": false}
            }}
        });
        let v = violations(&cognition(json!({"chat": chat}), json!({})));
        assert_eq!(
            v[0].path,
            "/interface/provides/chat/$defs/chat/attach/properties/reply"
        );
        assert!(v[0].message.contains("找不到"), "{}", v[0].message);
    }

    #[test]
    fn a_dangling_root_ref_is_reported() {
        let spec = json!({"$ref": "#/$defs/nope", "$defs": {"x": port_min()}});
        let v = violations(&cognition(json!({"p": spec}), json!({})));
        assert!(
            v.iter()
                .any(|v| v.path == "/interface/provides/p" && v.message.contains("找不到")),
            "{v:?}"
        );
    }

    #[test]
    fn false_in_properties_is_refused_but_false_as_the_default_is_fine() {
        let mut p = port_min();
        p["attach"] = json!({"properties": {"x": false}, "additionalProperties": false});
        let v = violations(&cognition(json!({"p": p}), json!({})));
        assert!(v[0].path.ends_with("/attach/properties/x"), "{}", v[0].path);
        // `additionalProperties: false` 本身是允许的，port_min 里就有。
        check_shape(&cognition(json!({"p": port_min()}), json!({}))).unwrap();
    }

    #[test]
    fn extra_fields_and_missing_fields_fail_the_definition_schema() {
        let mut extra = cognition(json!({}), json!({}));
        extra["surprise"] = json!(1);
        assert!(violations(&extra)[0].message.contains("anyOf"));
        let mut missing = cognition(json!({}), json!({}));
        missing.as_object_mut().unwrap().remove("body");
        violations(&missing);
    }

    #[test]
    fn a_charter_is_refused_until_its_checks_exist() {
        let iface = cognition(json!({}), json!({}))["interface"].clone();
        let charter = json!({
            "interface": iface,
            "seats": {"a": {"interface": iface}},
            "wire": {"a": {}},
            "export": {"signature": "a", "provides": {}}
        });
        let v = violations(&charter);
        assert!(v[0].message.contains("Charter"), "{}", v[0].message);
    }
}
