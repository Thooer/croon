//! 校验器：只认 value.schema 的子集。
//! 参考：docs/Kernel/Value.md（Schema）。
//!
//! 报错带路径，说出违反了哪条规则。先用 `check_schema` 校验一份 Schema（未列出的关键字被拒），
//! 再用 `validate` 校验值。

use std::fmt;
use std::sync::OnceLock;

use serde_json::Value as Json;

/// 一处违反：`path` 是 JSON Pointer，根是空串；`rule` 是违反的 value.schema 关键字，如 `type`、`required`，
/// 不是 Schema 本身的检查时是一个固定的短名，如 `shape`。Language 拼给模型看的报错只用这两样，
/// `message` 的措辞不随 Language 的版本冻结。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub rule: &'static str,
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "（根）：{}", self.message)
        } else {
            write!(f, "{}：{}", self.path, self.message)
        }
    }
}

/// 一次最多报这么多处，免得一份坏数据换来一份巨大的报告。
const MAX_VIOLATIONS: usize = 32;
/// `$ref` 链与嵌套的上限，挡住 `$ref` 指向自己的 Schema。
const MAX_DEPTH: usize = 512;

pub type Report = Result<(), Vec<Violation>>;

/// 值是否满足 Schema。Schema 要先过 `check_schema`；没过的，结果没有意义。
pub fn validate(schema: &Json, value: &Json) -> Report {
    let mut cx = Cx {
        root: schema,
        out: Vec::new(),
    };
    walk(&mut cx, schema, value, &mut String::new(), 0);
    if cx.out.is_empty() {
        Ok(())
    } else {
        Err(cx.out)
    }
}

struct Cx<'a> {
    root: &'a Json,
    out: Vec<Violation>,
}

impl Cx<'_> {
    fn full(&self) -> bool {
        self.out.len() >= MAX_VIOLATIONS
    }

    fn fail(&mut self, path: &str, rule: &'static str, message: String) {
        if !self.full() {
            self.out.push(Violation {
                path: path.to_owned(),
                rule,
                message,
            });
        }
    }
}

fn walk(cx: &mut Cx, schema: &Json, value: &Json, path: &mut String, depth: usize) {
    if cx.full() {
        return;
    }
    if depth > MAX_DEPTH {
        cx.fail(
            path,
            "$ref",
            "Schema 嵌套得太深，或 $ref 指回了自己".to_owned(),
        );
        return;
    }
    let Some(schema) = schema.as_object() else {
        cx.fail(path, "schema", "Schema 必须是对象".to_owned());
        return;
    };

    if let Some(kind) = schema.get("type").and_then(Json::as_str)
        && !kind_ok(kind, value)
    {
        cx.fail(
            path,
            "type",
            format!("应为 {kind}，实际是 {}", describe(value)),
        );
    }
    if let Some(Json::Array(allowed)) = schema.get("enum")
        && !allowed.iter().any(|a| json_eq(a, value))
    {
        let list: Vec<String> = allowed.iter().map(Json::to_string).collect();
        cx.fail(
            path,
            "enum",
            format!("应为 {} 之一，实际是 {value}", list.join("、")),
        );
    }
    if let Some(r) = schema.get("$ref").and_then(Json::as_str) {
        match resolve(cx.root, r) {
            Some(target) => walk(cx, target, value, path, depth + 1),
            None => cx.fail(path, "$ref", format!("$ref {r} 指不到顶层的 $defs")),
        }
    }
    if let Some(Json::Array(branches)) = schema.get("anyOf") {
        any_of(cx, branches, value, path, depth);
    }
    match value {
        Json::Object(map) => object(cx, schema, map, path, depth),
        Json::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (i, item) in items.iter().enumerate() {
                    let mark = path.len();
                    path.push_str(&format!("/{i}"));
                    walk(cx, item_schema, item, path, depth + 1);
                    path.truncate(mark);
                }
            }
        }
        _ => {}
    }
}

fn any_of(cx: &mut Cx, branches: &[Json], value: &Json, path: &mut String, depth: usize) {
    // 没有一支满足时，报最接近的那一支：类型就对不上的比类型对得上、里面缺东西的更远；
    // 同一档里违反处越少越近。模型靠这一条报错改正，所以要指到真正该改的地方。
    let mut nearest: Option<(bool, Vec<Violation>)> = None;
    for branch in branches {
        let mut sub = Cx {
            root: cx.root,
            out: Vec::new(),
        };
        walk(&mut sub, branch, value, path, depth + 1);
        if sub.out.is_empty() {
            return;
        }
        let wrong_kind = sub.out.iter().any(|v| {
            v.path == *path && v.message.starts_with("应为 ") && !v.message.contains("之一")
        });
        let better = nearest
            .as_ref()
            .is_none_or(|(n_kind, n)| (wrong_kind, sub.out.len()) < (*n_kind, n.len()));
        if better {
            nearest = Some((wrong_kind, sub.out));
        }
    }
    let detail = nearest
        .and_then(|(_, n)| n.into_iter().next())
        .map_or_else(String::new, |v| format!("；最接近的一支：{v}"));
    cx.fail(
        path,
        "anyOf",
        format!(
            "不满足 anyOf 的任何一支（共 {} 支）{detail}",
            branches.len()
        ),
    );
}

fn object(
    cx: &mut Cx,
    schema: &serde_json::Map<String, Json>,
    map: &serde_json::Map<String, Json>,
    path: &mut String,
    depth: usize,
) {
    let properties = schema.get("properties").and_then(Json::as_object);
    if let Some(Json::Array(required)) = schema.get("required") {
        for name in required.iter().filter_map(Json::as_str) {
            if !map.contains_key(name) {
                cx.fail(path, "required", format!("缺少必需的字段 `{name}`"));
            }
        }
    }
    for (key, value) in map {
        let mark = path.len();
        path.push('/');
        path.push_str(&key.replace('~', "~0").replace('/', "~1"));
        match properties.and_then(|p| p.get(key)) {
            Some(sub) => walk(cx, sub, value, path, depth + 1),
            None => match schema.get("additionalProperties") {
                Some(Json::Bool(false)) => cx.fail(
                    path,
                    "additionalProperties",
                    format!("不允许的字段 `{key}`"),
                ),
                Some(sub @ Json::Object(_)) => walk(cx, sub, value, path, depth + 1),
                _ => {}
            },
        }
        path.truncate(mark);
    }
}

fn kind_ok(kind: &str, value: &Json) -> bool {
    match kind {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.as_number().is_some_and(is_integral),
        "string" => value.is_string(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn is_integral(n: &serde_json::Number) -> bool {
    n.is_i64() || n.is_u64() || n.as_f64().is_some_and(|x| x.fract() == 0.0)
}

fn describe(value: &Json) -> String {
    match value {
        Json::Null => "null".into(),
        Json::Bool(_) => "boolean".into(),
        Json::Number(n) if !is_integral(n) => format!("带小数的数 {n}"),
        Json::Number(_) => "number".into(),
        Json::String(_) => "string".into(),
        Json::Array(_) => "array".into(),
        Json::Object(_) => "object".into(),
    }
}

/// 按 JSON 相等比较：数按值比（1 与 1.0 相等），对象与键的顺序无关。
pub fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => match (x.as_i64(), y.as_i64()) {
            (Some(i), Some(j)) => i == j,
            _ => x.as_f64() == y.as_f64(),
        },
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q))
        }
        (Json::Object(x), Json::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

/// `#/$defs/名字` 在顶层的位置；别的写法解析不到。
fn resolve<'a>(root: &'a Json, reference: &str) -> Option<&'a Json> {
    let name = reference.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

/// 一份 Schema 是否合法：先满足 value.schema（未列出的关键字被拒），
/// 再满足 `$ref` 只指向顶层的 `$defs`、`$defs` 只写在顶层。
pub fn check_schema(schema: &Json) -> Report {
    static META: OnceLock<Json> = OnceLock::new();
    let meta = META.get_or_init(|| crate::blocks::json("value.schema"));
    validate(meta, schema)?;
    let mut out = Vec::new();
    check_refs(schema, schema, true, &mut String::new(), &mut out);
    if out.is_empty() { Ok(()) } else { Err(out) }
}

fn check_refs(
    node: &Json,
    root: &Json,
    is_root: bool,
    path: &mut String,
    out: &mut Vec<Violation>,
) {
    let Some(node) = node.as_object() else { return };
    let mut fail = |path: &str, rule: &'static str, message: String| {
        if out.len() < MAX_VIOLATIONS {
            out.push(Violation {
                path: path.to_owned(),
                rule,
                message,
            });
        }
    };
    if node.contains_key("$defs") && !is_root {
        fail(path, "$defs", "$defs 只能写在顶层".to_owned());
    }
    if let Some(r) = node.get("$ref").and_then(Json::as_str) {
        if !r.starts_with("#/$defs/") {
            fail(
                path,
                "$ref",
                format!("$ref {r} 只能写作 #/$defs/名字，指向同一份 Schema 的根"),
            );
        } else if resolve(root, r).is_none() {
            fail(path, "$ref", format!("$ref {r} 在根的 $defs 里找不到"));
        }
    }

    let mut descend = |child: &Json, segment: String, out: &mut Vec<Violation>| {
        let mark = path.len();
        path.push_str(&segment);
        check_refs(child, root, false, path, out);
        path.truncate(mark);
    };
    if let Some(items) = node.get("items") {
        descend(items, "/items".into(), out);
    }
    if let Some(Json::Object(props)) = node.get("properties") {
        for (name, sub) in props {
            descend(
                sub,
                format!("/properties/{}", name.replace('~', "~0").replace('/', "~1")),
                out,
            );
        }
    }
    if let Some(extra @ Json::Object(_)) = node.get("additionalProperties") {
        descend(extra, "/additionalProperties".into(), out);
    }
    if let Some(Json::Array(branches)) = node.get("anyOf") {
        for (i, sub) in branches.iter().enumerate() {
            descend(sub, format!("/anyOf/{i}"), out);
        }
    }
    if is_root && let Some(Json::Object(defs)) = node.get("$defs") {
        for (name, sub) in defs {
            descend(
                sub,
                format!("/$defs/{}", name.replace('~', "~0").replace('/', "~1")),
                out,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks;
    use serde_json::json;

    fn ok(schema: Json, value: Json) {
        if let Err(v) = validate(&schema, &value) {
            panic!("应当满足，却违反了：{v:?}\nschema = {schema}\nvalue = {value}");
        }
    }

    fn fails(schema: Json, value: Json) -> Vec<Violation> {
        validate(&schema, &value)
            .expect_err(&format!("应当违反：schema = {schema}, value = {value}"))
    }

    #[test]
    fn an_empty_schema_accepts_anything() {
        for v in [
            json!(null),
            json!(1),
            json!("x"),
            json!([1]),
            json!({"a": 1}),
        ] {
            ok(json!({}), v);
        }
    }

    #[test]
    fn type_is_checked_and_integer_is_a_number_with_an_integral_value() {
        ok(json!({"type": "integer"}), json!(3));
        ok(json!({"type": "integer"}), json!(1.0));
        ok(json!({"type": "number"}), json!(1.5));
        ok(json!({"type": "number"}), json!(2));
        let v = fails(json!({"type": "integer"}), json!(1.5));
        assert_eq!(v[0].message, "应为 integer，实际是 带小数的数 1.5");
        fails(json!({"type": "string"}), json!(1));
        fails(json!({"type": "null"}), json!(false));
        fails(json!({"type": "array"}), json!({}));
    }

    #[test]
    fn enum_compares_by_json_equality() {
        ok(json!({"enum": [1, "a", null]}), json!(1.0));
        ok(json!({"enum": [{"x": [1]}]}), json!({"x": [1.0]}));
        fails(json!({"enum": ["a", "b"]}), json!("c"));
    }

    #[test]
    fn required_properties_and_additional_properties() {
        let schema = json!({
            "type": "object",
            "properties": {"a": {"type": "integer"}, "b": {"type": "string"}},
            "required": ["a"],
            "additionalProperties": false
        });
        ok(schema.clone(), json!({"a": 1}));
        ok(schema.clone(), json!({"a": 1, "b": "x"}));
        assert_eq!(
            fails(schema.clone(), json!({"b": "x"}))[0].message,
            "缺少必需的字段 `a`"
        );
        assert_eq!(fails(schema.clone(), json!({"a": 1, "c": 2}))[0].path, "/c");
        let v = fails(schema, json!({"a": "no"}));
        assert_eq!(v[0].path, "/a");
    }

    #[test]
    fn additional_properties_can_be_a_schema_for_maps() {
        let schema = json!({"type": "object", "additionalProperties": {"type": "integer"}});
        ok(schema.clone(), json!({"x": 1, "y": 2}));
        assert_eq!(fails(schema, json!({"x": 1, "y": "z"}))[0].path, "/y");
    }

    #[test]
    fn without_additional_properties_other_fields_are_free() {
        ok(
            json!({"properties": {"a": {"type": "integer"}}}),
            json!({"a": 1, "zzz": "anything"}),
        );
    }

    #[test]
    fn properties_and_items_only_constrain_their_own_kind_of_value() {
        ok(
            json!({"properties": {"a": {"type": "integer"}}, "items": {"type": "string"}}),
            json!(5),
        );
    }

    #[test]
    fn items_are_checked_with_an_index_path() {
        let v = fails(
            json!({"type": "array", "items": {"type": "integer"}}),
            json!([1, 2, "x"]),
        );
        assert_eq!(v[0].path, "/2");
    }

    #[test]
    fn nested_paths_escape_slashes_and_tildes() {
        let schema = json!({"properties": {"a/b": {"properties": {"~": {"type": "integer"}}}}});
        assert_eq!(
            fails(schema, json!({"a/b": {"~": "x"}}))[0].path,
            "/a~1b/~0"
        );
    }

    #[test]
    fn any_of_accepts_one_branch_and_names_the_nearest_when_none_fit() {
        let schema = json!({"anyOf": [{"type": "integer"}, {"type": "object", "required": ["k"]}]});
        ok(schema.clone(), json!(1));
        ok(schema.clone(), json!({"k": 1}));
        let v = fails(schema, json!({"j": 1}));
        assert!(
            v[0].message
                .starts_with("不满足 anyOf 的任何一支（共 2 支）"),
            "{}",
            v[0].message
        );
        assert!(
            v[0].message.contains("缺少必需的字段 `k`"),
            "{}",
            v[0].message
        );
    }

    #[test]
    fn keywords_in_one_schema_all_hold() {
        let schema = json!({"type": "integer", "enum": [1, 2]});
        ok(schema.clone(), json!(1));
        fails(schema.clone(), json!(3));
        fails(schema, json!("1"));
    }

    #[test]
    fn ref_resolves_against_the_root_defs_and_supports_recursion() {
        let schema = json!({
            "$ref": "#/$defs/tree",
            "$defs": {"tree": {"type": "object", "additionalProperties": {"anyOf": [
                {"type": "integer"},
                {"$ref": "#/$defs/tree"}
            ]}}}
        });
        ok(schema.clone(), json!({"a": 1, "b": {"c": {"d": 2}}}));
        // 嵌套的 anyOf 把内层的失败包进外层那一条：报在外层所在的位置，最深处的路径在说明里。
        let v = fails(schema, json!({"a": {"b": {"c": "x"}}}));
        assert_eq!(v[0].path, "/a");
        assert!(v[0].message.contains("/a/b/c"), "{}", v[0].message);
    }

    #[test]
    fn a_ref_that_points_at_itself_fails_instead_of_looping() {
        let schema = json!({"$ref": "#/$defs/a", "$defs": {"a": {"$ref": "#/$defs/a"}}});
        let v = fails(schema, json!(1));
        assert!(v[0].message.contains("太深"), "{}", v[0].message);
    }

    #[test]
    fn a_report_is_bounded() {
        let value = Json::Array((0..1000).map(|_| json!("x")).collect());
        let v = fails(json!({"items": {"type": "integer"}}), value);
        assert_eq!(v.len(), MAX_VIOLATIONS);
    }

    // ---- check_schema ----

    fn bad(schema: Json) -> Vec<Violation> {
        check_schema(&schema).expect_err(&format!("应当不是合法的 Schema：{schema}"))
    }

    #[test]
    fn a_valid_schema_passes() {
        check_schema(
            &json!({"type": "object", "properties": {"a": {"type": "integer"}}, "required": ["a"]}),
        )
        .unwrap();
    }

    #[test]
    fn unknown_keywords_and_unknown_types_are_rejected() {
        let v = bad(json!({"pattern": "x"}));
        assert!(
            v[0].message.contains("不允许的字段 `pattern`"),
            "{}",
            v[0].message
        );
        bad(json!({"type": "str"}));
        bad(json!({"properties": {"a": {"minimum": 1}}}));
        bad(json!({"type": ["string", "null"]}));
    }

    #[test]
    fn ref_must_point_at_the_root_defs_of_the_same_schema() {
        bad(json!({"$ref": "#/$defs/missing"}));
        bad(json!({"$ref": "other.json#/$defs/a", "$defs": {"a": {}}}));
        bad(json!({"$ref": "#/properties/a", "properties": {"a": {}}}));
        check_schema(&json!({"$ref": "#/$defs/a", "$defs": {"a": {"type": "string"}}})).unwrap();
    }

    #[test]
    fn defs_may_only_sit_at_the_root() {
        let v = bad(json!({"properties": {"a": {"$defs": {"x": {}}}}}));
        assert!(
            v.iter().any(|v| v.message.contains("只能写在顶层")),
            "{v:?}"
        );
    }

    // ---- 本文自己的 Schema：文档与校验器互为证据 ----

    #[test]
    fn value_schema_is_a_schema_and_describes_itself() {
        let meta = blocks::json("value.schema");
        check_schema(&meta).unwrap();
        validate(&meta, &meta).unwrap();
    }
}
