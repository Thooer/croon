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
        charter(definition, &|_| None, &mut out);
        return if out.is_empty() { Ok(()) } else { Err(out) };
    }
    interface(&definition["interface"], "/interface", &mut out);
    if out.is_empty() { Ok(()) } else { Err(out) }
}

/// Charter 的构造检查：见 docs/Kernel/Definition/README.md 的「Charter」的「检查」。这里查得出的
/// 是纯结构的那几条；写死的 Definition 满足它的 Seat 要读闭包与构造事实，在 Kernel 侧另查。
///
/// `kernel` 给出 Kernel 自己那几个 Port 的规格（look、notify、correct）。它不在这一层，所以由
/// 调用方传入；返回 None 表示这时拿不到，与它们形状兼容的那几条就跳过，其余照查。
pub fn check_charter(definition: &Json, kernel: &dyn Fn(&str) -> Option<Json>) -> Report {
    let mut out = Vec::new();
    charter(definition, kernel, &mut out);
    if out.is_empty() { Ok(()) } else { Err(out) }
}

/// Charter 的结构检查：至少两个 Seat；每个 Seat 的 Interface；每个 Required port 恰好接一种
/// 来源，来源指得到、形状兼容；导出的形状兼容；没有成员接上自己 Seat 的 Correct。
fn charter(def: &Json, kernel: &dyn Fn(&str) -> Option<Json>, out: &mut Vec<Violation>) {
    interface(&def["interface"], "/interface", out);
    let seats = def["seats"].as_object().cloned().unwrap_or_default();
    if seats.len() < 2 {
        fail(
            out,
            "/seats",
            format!(
                "至少要两个 Seat，现在有 {}：只有一个 Seat 的 Charter 与直接实例化那份 Definition 承担同样的保证",
                seats.len()
            ),
        );
    }
    for (name, seat) in &seats {
        let path = format!("/seats/{}", escape(name));
        if name.is_empty() || name.contains('/') {
            fail(out, &path, "Seat 的名字非空，不含 /".to_owned());
        }
        interface(&seat["interface"], &path, out);
    }

    let wire = def["wire"].as_object().cloned().unwrap_or_default();
    for (name, ports) in &wire {
        let path = format!("/wire/{}", escape(name));
        let Some(seat) = seats.get(name) else {
            fail(out, &path, format!("没有名为 {name} 的 Seat"));
            continue;
        };
        let requires = seat["interface"]["requires"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        for (port, source) in ports.as_object().into_iter().flatten() {
            let at = format!("{path}/{}", escape(port));
            let Some(decl) = requires.get(port) else {
                fail(out, &at, format!("这个 Seat 没有 Required port {port}"));
                continue;
            };
            match source_spec(def, &seats, name, source, kernel, &at, out) {
                Some(Some(spec)) => {
                    if !crate::port::narrows(decl, &spec) {
                        fail(
                            out,
                            &at,
                            "接收方写的声明要比来源的更窄或相同，否则构造时看不出能接上".to_owned(),
                        );
                    }
                }
                Some(None) => {}
                None => {}
            }
        }
        for port in requires.keys() {
            let wired = ports.get(port).is_some();
            if !wired {
                fail(
                    out,
                    &format!("{path}/{}", escape(port)),
                    "每个 Required port 恰好接一种来源".to_owned(),
                );
            }
        }
    }
    for (name, seat) in &seats {
        if wire.contains_key(name) {
            continue;
        }
        for port in seat["interface"]["requires"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(p, _)| p)
        {
            fail(
                out,
                &format!("/wire/{}/{}", escape(name), escape(port)),
                "每个 Required port 恰好接一种来源".to_owned(),
            );
        }
    }

    let export = &def["export"];
    let signature = export["signature"].as_str().unwrap_or_default();
    if !seats.contains_key(signature) {
        fail(
            out,
            "/export/signature",
            format!("没有名为 {signature} 的 Seat"),
        );
    }
    let provides = export["provides"].as_object().cloned().unwrap_or_default();
    let mine = def["interface"]["provides"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for (port, to) in &provides {
        let at = format!("/export/provides/{}", escape(port));
        let Some(decl) = mine.get(port) else {
            fail(out, &at, format!("Collective 没有 Provided port {port}"));
            continue;
        };
        let from = to["seat"].as_str().unwrap_or_default();
        let target = to["port"].as_str().unwrap_or_default();
        let Some(seat) = seats.get(from) else {
            fail(out, &at, format!("没有名为 {from} 的 Seat"));
            continue;
        };
        let Some(spec) = seat["interface"]["provides"].get(target) else {
            fail(out, &at, format!("这个 Seat 没有 Provided port {target}"));
            continue;
        };
        if !crate::port::narrows(decl, spec) {
            fail(
                out,
                &at,
                "Collective 的 Interface 不比这个 Seat 的更宽：外面的一方只能按它交出".to_owned(),
            );
        }
    }
    for port in mine.keys() {
        if !provides.contains_key(port) {
            fail(
                out,
                &format!("/export/provides/{}", escape(port)),
                "Collective 的每个 Provided port 接到某个 Seat".to_owned(),
            );
        }
    }

    if let Some(pin) = seats.values().filter_map(|s| s.get("pin")).next()
        && pin["definition"].as_str().is_none_or(str::is_empty)
    {
        fail(out, "/seats", "pin 要写完整 Definition Bundle ID".to_owned());
    }
}

/// 一种来源的 Port spec。`Some(None)` 是这时拿不到（Kernel 的那几个），跳过形状比较；
/// `None` 是已经报了错。
fn source_spec(
    def: &Json,
    seats: &serde_json::Map<String, Json>,
    seat: &str,
    source: &Json,
    kernel: &dyn Fn(&str) -> Option<Json>,
    at: &str,
    out: &mut Vec<Violation>,
) -> Option<Option<Json>> {
    if let Some(outer) = source.get("outer").and_then(Json::as_str) {
        return match def["interface"]["requires"].get(outer) {
            Some(spec) => Some(Some(spec.clone())),
            None => {
                fail(out, at, format!("Collective 没有 Required port {outer}"));
                None
            }
        };
    }
    if let Some(other) = source.get("seat").and_then(Json::as_str) {
        let p = source["port"].as_str().unwrap_or_default();
        let Some(target) = seats.get(other) else {
            fail(out, at, format!("没有名为 {other} 的 Seat"));
            return None;
        };
        return match target["interface"]["provides"].get(p) {
            Some(spec) => Some(Some(spec.clone())),
            None => {
                fail(out, at, format!("Seat {other} 没有 Provided port {p}"));
                None
            }
        };
    }
    if let Some(own) = source.get("self").and_then(Json::as_str) {
        return match seats.get(seat).and_then(|s| s["interface"]["provides"].get(own)) {
            Some(spec) => Some(Some(spec.clone())),
            None => {
                fail(out, at, format!("这个 Seat 没有 Provided port {own}"));
                None
            }
        };
    }
    for op in ["look", "notify", "correct"] {
        if let Some(target) = source.get(op).and_then(Json::as_str) {
            if !seats.contains_key(target) {
                fail(out, at, format!("没有名为 {target} 的 Seat"));
                return None;
            }
            if op == "correct" && target == seat {
                fail(out, at, "没有成员接上自己 Seat 的 Correct".to_owned());
                return None;
            }
            return Some(kernel(op));
        }
    }
    fail(out, at, "来源要恰好写一种".to_owned());
    None
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

    fn iface(provides: Json, requires: Json) -> Json {
        json!({
            "description": "接口",
            "signature": {"in": {"type": "object"}, "out": {"type": "string"}},
            "provides": provides,
            "requires": requires
        })
    }

    /// 两个 Seat 的最小 Charter：门的 Required port 接 Collective 的，审计者接门的 Provided port。
    fn chartered(seat_requires: Json, collective_requires: Json) -> Json {
        json!({
            "interface": iface(json!({}), collective_requires),
            "seats": {
                "gate": {"interface": iface(json!({"pass": port_min()}), seat_requires)},
                "audit": {"interface": iface(json!({}), json!({"see": port_min()}))}
            },
            "wire": {
                "gate": {"up": {"outer": "run"}},
                "audit": {"see": {"seat": "gate", "port": "pass"}}
            },
            "export": {"signature": "gate", "provides": {}}
        })
    }

    fn valid_charter() -> Json {
        chartered(json!({"up": port_min()}), json!({"run": port_min()}))
    }

    #[test]
    fn a_charter_with_two_seats_and_a_wired_graph_passes() {
        check_shape(&valid_charter()).unwrap();
    }

    #[test]
    fn a_charter_needs_at_least_two_seats() {
        let mut one = valid_charter();
        let seats = one["seats"].as_object_mut().unwrap();
        seats.remove("audit");
        let _ = seats
            .get_mut("gate")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("interface");
        one["seats"]["gate"]["interface"] = iface(json!({"pass": port_min()}), json!({"up": port_min()}));
        one["wire"] = json!({"gate": {"up": {"outer": "run"}}});
        one["export"]["signature"] = json!("gate");
        let v = violations(&one);
        assert!(v.iter().any(|x| x.message.contains("至少要两个 Seat")), "{v:?}");
    }

    #[test]
    fn every_required_port_is_wired_exactly_once() {
        let mut missing = valid_charter();
        missing["wire"]["audit"] = json!({});
        let v = violations(&missing);
        assert!(
            v.iter().any(|x| x.path == "/wire/audit/see"),
            "缺一条来源要报到那个 Port 上：{v:?}"
        );
        let mut unknown = valid_charter();
        unknown["wire"]["audit"]["see"] = json!({"seat": "nobody", "port": "pass"});
        assert!(
            violations(&unknown)
                .iter()
                .any(|x| x.message.contains("nobody"))
        );
    }

    #[test]
    fn a_receiver_declaration_narrows_the_source_it_comes_from() {
        let full = json!({"anyOf": [
            {"type": "object", "properties": {"op": {"enum": ["pause"]}}, "required": ["op"]},
            {"type": "object", "properties": {"op": {"enum": ["resume"]}}, "required": ["op"]}
        ]});
        let pause = json!({"type": "object", "properties": {"op": {"enum": ["pause"]}}, "required": ["op"]});
        let port_of = |inv: &Json| json!({"description": "d", "in": inv, "attach": {"additionalProperties": false}});

        // 来源只开放 pause，接收方只收 pause：通过。
        let narrower = chartered(json!({"up": port_of(&pause)}), json!({"run": port_of(&pause)}));
        check_shape(&narrower).unwrap_or_else(|e| panic!("只收一半是通过的：{e:?}"));

        // 来源只开放 pause，接收方声明整份：比来源宽，构造时报在那一节上。
        let wider = chartered(json!({"up": port_of(&full)}), json!({"run": port_of(&pause)}));
        let v = violations(&wider);
        assert!(v[0].path.contains("/wire/gate/up"), "{v:?}");
    }

    #[test]
    fn a_member_cannot_be_wired_to_its_own_seats_correct() {
        let mut c = valid_charter();
        c["wire"]["audit"]["see"] = json!({"correct": "audit"});
        let v = violations(&c);
        assert!(
            v.iter().any(|x| x.message.contains("自己 Seat 的 Correct")),
            "{v:?}"
        );
    }

    #[test]
    fn an_export_cannot_be_wider_than_the_seat_it_points_at() {
        let mut c = valid_charter();
        c["interface"]["provides"] = json!({"run": port_min()});
        c["export"]["provides"] = json!({"run": {"seat": "gate", "port": "pass"}});
        check_shape(&c).unwrap_or_else(|e| panic!("导出一份相同的 Port 是通过的：{e:?}"));

        let mut wider = c.clone();
        wider["interface"]["provides"]["run"] = json!({
            "description": "更宽", "in": {}, "attach": {"additionalProperties": true}
        });
        let v = violations(&wider);
        assert!(v[0].path.contains("/export/provides/run"), "{v:?}");
    }
}
