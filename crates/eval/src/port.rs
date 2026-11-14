//! 两份 Port spec 的比较：相同，与收窄。参考：docs/Kernel/Definition/README.md「Interface 与
//! Implementation」。
//!
//! 收窄只比能枚举的两处：`in` 顶层的分支，与附带声明里的名字。分支内部与叶子按相同比，不做
//! 通用的包含判断——中间通配这类等价于正则语言的包含，判不准，也就不该写进通道能强制的地方。
//! 这里不知道 Kernel 的 Port 表，也不知道 Instance；`look` 这类来源的规格由调用方给。

use serde_json::Value as Json;

use crate::schema::json_eq;

/// 去掉各处的 description：字符串值的 description 是描述，对象值的是名为 description 的字段，留着。
pub fn strip(v: &Json) -> Json {
    match v {
        Json::Object(m) => Json::Object(
            m.iter()
                .filter(|(k, v)| !(k.as_str() == "description" && v.is_string()))
                .map(|(k, v)| (k.clone(), strip(v)))
                .collect(),
        ),
        Json::Array(a) => Json::Array(a.iter().map(strip).collect()),
        other => other.clone(),
    }
}

/// 一份 Port spec 相同：去掉各处的 description 之后规范文本相同。
pub fn same(a: &Json, b: &Json) -> bool {
    json_eq(&strip(a), &strip(b))
}

/// `a` 是 `b` 的收窄：`a` 能做的都在 `b` 之内。`a` 是接收方写的声明，`b` 是派生链上的一节。
///
/// - `in`：`a` 的每一支（有 anyOf 就按它分，没有就整份算一支）都在 `b` 的某一支里相同。
/// - 附带：`a` 写明的名字都在 `b` 允许之内，同名的叶子按同一条递归；`b` 的
///   additionalProperties 是 true 时其余名字都不限，是 false 时 `a` 也只能是 false。
/// - `out`：两边都有时相同。Provided port 没有 out，接收方的声明也不写。
///
/// `required` 不比：接收方多要一些，是它自己承诺得更严，不超过它持有的；少要一些，交出的一方
/// 仍要满足链上其他节的 required。所以比不比都不改变授权。
pub fn narrows(a: &Json, b: &Json) -> bool {
    let (a, b) = (deref(a), deref(b));
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    if !in_narrows(&a["in"], &b["in"]) {
        return false;
    }
    if !attach_narrows(&a["attach"], &b["attach"]) {
        return false;
    }
    match (a.get("out"), b.get("out")) {
        (None, _) => true,
        (Some(x), Some(y)) => same(x, y),
        (Some(_), None) => false,
    }
}

/// 顶层的 `$ref` 沿着自己那份 `$defs` 解开，最多 8 跳。
fn deref(v: &Json) -> Option<Json> {
    let mut here = v.clone();
    for _ in 0..8 {
        let Some(r) = here.get("$ref").and_then(Json::as_str) else {
            return Some(here);
        };
        if here.get("in").is_some() {
            return Some(here);
        }
        let name = r.strip_prefix("#/$defs/")?;
        here = here.get("$defs")?.get(name)?.clone();
    }
    None
}

fn branches(schema: &Json) -> Vec<Json> {
    match schema.get("anyOf").and_then(Json::as_array) {
        Some(a) => a.iter().map(|b| deref(b).unwrap_or_else(|| b.clone())).collect(),
        None => vec![deref(schema).unwrap_or_else(|| schema.clone())],
    }
}

fn in_narrows(a: &Json, b: &Json) -> bool {
    let b = branches(b);
    branches(a)
        .iter()
        .all(|x| b.iter().any(|y| same(x, y)))
}

fn attach_narrows(a: &Json, b: &Json) -> bool {
    let props = |v: &Json| {
        v.get("properties")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let (pa, pb) = (props(a), props(b));
    for (name, leaf) in &pa {
        match pb.get(name) {
            Some(other) => {
                if !leaf_narrows(leaf, other) {
                    return false;
                }
            }
            None => match b.get("additionalProperties") {
                Some(Json::Bool(true)) => {}
                Some(other) => {
                    if !leaf_narrows(leaf, other) {
                        return false;
                    }
                }
                None => {}
            },
        }
    }
    match (a.get("additionalProperties"), b.get("additionalProperties")) {
        (Some(Json::Bool(false)), _) => true,
        (Some(x), Some(Json::Bool(true))) => !matches!(x, Json::Bool(_)),
        (Some(x), Some(y)) => leaf_narrows(x, y),
        (Some(x), None) => !matches!(x, Json::Bool(_)),
        (None, _) => true,
    }
}

/// 附带的叶子：`true` 是任意的上限，`false` 是不接受；其余是一份 Port spec 或指向它的 `$ref`。
fn leaf_narrows(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Bool(true), Json::Bool(true)) => true,
        (Json::Bool(false), _) => true,
        (_, Json::Bool(true)) => !matches!(a, Json::Bool(_)),
        (Json::Bool(_), _) => false,
        (Json::Object(m), _) if m.contains_key("$ref") && !m.contains_key("in") => {
            match (deref(a), deref(b)) {
                (Some(x), Some(y)) => narrows(&x, &y),
                _ => false,
            }
        }
        _ => narrows(a, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn port(inv: Json, attach: Json) -> Json {
        json!({"description": "d", "in": inv, "attach": attach})
    }

    fn closed() -> Json {
        json!({"additionalProperties": false})
    }

    #[test]
    fn a_port_is_its_own_narrowing_and_description_does_not_count() {
        let a = port(json!({"type": "null"}), closed());
        let mut b = a.clone();
        b["description"] = json!("别的说法");
        assert!(narrows(&a, &b));
        assert!(narrows(&b, &a), "description 不同的两份互为收窄");
    }

    #[test]
    fn one_branch_of_an_any_of_is_a_narrowing_of_it() {
        let full = port(
            json!({"anyOf": [
                {"type": "object", "properties": {"op": {"enum": ["pause"]}}, "required": ["op"]},
                {"type": "object", "properties": {"op": {"enum": ["resume"]}}, "required": ["op"]}
            ]}),
            closed(),
        );
        let pause = port(
            json!({"type": "object", "properties": {"op": {"enum": ["pause"]}}, "required": ["op"]}),
            closed(),
        );
        assert!(narrows(&pause, &full));
        assert!(!narrows(&full, &pause), "更宽的不是收窄");
    }

    #[test]
    fn dropping_a_name_from_the_attach_is_a_narrowing_but_adding_one_is_not() {
        let one = json!({"properties": {"to": true}, "required": ["to"], "additionalProperties": false});
        let two = json!({"properties": {"to": true, "extra": true}, "additionalProperties": false});
        assert!(narrows(&port(json!({}), one.clone()), &port(json!({}), two.clone())));
        assert!(!narrows(&port(json!({}), two), &port(json!({}), one)));
    }

    #[test]
    fn a_leaf_that_is_not_written_may_still_be_allowed_by_the_wider_side() {
        let open = json!({"additionalProperties": true});
        let named = json!({"properties": {"to": true}, "additionalProperties": false});
        assert!(narrows(&port(json!({}), named), &port(json!({}), open)));
    }
}
