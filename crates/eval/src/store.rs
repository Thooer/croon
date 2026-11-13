//! Bundle 的形状：内容与显式依赖。存储与交付资格在 `croon` 的 store 里。
//! 参考：docs/Kernel/Value.md（Bundle）。

use serde_json::Value as Json;

use crate::value::ValueId;

/// 一份 Bundle，形状满足 `value.bundle`。ID 即描述本身的内容 ID。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    pub content: String,
    pub dependencies: Vec<String>,
}

/// 按 `value.bundle` 解析一份 JSON。不合形状、依赖未排序或有重复时返回 `None`。
pub fn parse(json: &Json) -> Option<Bundle> {
    if crate::schema::validate(&crate::blocks::json("value.bundle"), json).is_err() {
        return None;
    }
    let content = json["content"].as_str()?.to_owned();
    let dependencies: Vec<String> = json["dependencies"]
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    if ValueId::parse(&content).is_none()
        || dependencies.iter().any(|d| ValueId::parse(d).is_none())
        || dependencies.windows(2).any(|w| w[0] >= w[1])
    {
        return None;
    }
    Some(Bundle {
        content,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn id(c: char) -> String {
        format!("sha256:{}", c.to_string().repeat(64))
    }

    #[test]
    fn a_bundle_parses_and_its_dependencies_must_be_sorted_and_unique() {
        let a = id('a');
        let b = id('b');
        assert!(parse(&json!({"content": a, "dependencies": []})).is_some());
        assert!(parse(&json!({"content": a, "dependencies": [b.clone(), a.clone()]})).is_none());
        assert!(parse(&json!({"content": a, "dependencies": [a.clone(), a.clone()]})).is_none());
        assert!(parse(&json!({"content": a, "dependencies": [a.clone()]})).is_some());
    }

    #[test]
    fn a_shape_violation_is_not_a_bundle() {
        assert!(parse(&json!({"content": "nope", "dependencies": []})).is_none());
        assert!(parse(&json!({"content": id('a')})).is_none());
        assert!(parse(&json!({"content": id('a'), "dependencies": [], "x": 1})).is_none());
    }
}
