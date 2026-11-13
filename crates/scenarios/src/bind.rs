//! 占位名的绑定：同一个名字只绑一个 ID，不同的名字不绑同一个；比对时在绑定下逐字段相等。
//! 参考：docs/Kernel/Scenarios/README.md（格式、脚手架）。

use std::collections::{BTreeMap, BTreeSet};

use kernel::ids::kind_of;
use kernel::schema::json_eq;
use kernel::value::ValueId;
use serde_json::{Value as Json, json};

pub fn is_value_ph(s: &str) -> bool {
    s.starts_with("sha256:") && ValueId::parse(s).is_none()
}

pub fn is_ph(s: &str) -> bool {
    is_value_ph(s) || kind_of(s).is_some()
}

fn is_failure(m: &serde_json::Map<String, Json>) -> bool {
    m.len() == 3 && m.contains_key("kind") && m.contains_key("message") && m.contains_key("data")
}

#[derive(Default)]
pub struct Bind {
    /// Value 的占位名 → 展开过的内容。
    pub values: BTreeMap<String, Json>,
    pub fwd: BTreeMap<String, String>,
    pub back: BTreeMap<String, String>,
    /// 脚手架算出的 Value：ID → 内容，要导入内容库。
    pub made: BTreeMap<String, Json>,
    pub bytes: BTreeMap<String, Vec<u8>>,
}

impl Bind {
    pub fn new(values: BTreeMap<String, Json>) -> Bind {
        Bind {
            values,
            ..Bind::default()
        }
    }

    pub fn bind(&mut self, ph: &str, actual: &str) -> Result<(), String> {
        if let Some(a) = self.fwd.get(ph)
            && a != actual
        {
            return Err(format!("{ph} 已绑定到 {a}，这里是 {actual}"));
        }
        if let Some(p) = self.back.get(actual)
            && p != ph
        {
            return Err(format!(
                "{actual} 已绑定到 {p}，这里是 {ph}：内容相同的 Value 要用同一个名字"
            ));
        }
        self.fwd.insert(ph.to_owned(), actual.to_owned());
        self.back.insert(actual.to_owned(), ph.to_owned());
        Ok(())
    }

    /// Value 的占位名 → ID。写在 values 里的由内容算出；没写的，由脚手架造成 `{名字: {"link": 名字}}`。
    pub fn vid(&mut self, ph: &str) -> String {
        if let Some(a) = self.fwd.get(ph) {
            return a.clone();
        }
        if self.back.contains_key(ph) {
            return ph.to_owned();
        }
        if self.made.contains_key(ph) || self.bytes.contains_key(ph) {
            self.bind(ph, ph).unwrap();
            return ph.to_owned();
        }
        let content = match self.values.get(ph).cloned() {
            Some(c) => self.subst(&c),
            None => panic!("{ph} 没有声明内容；场景必须明确构造每份内容及依赖"),
        };
        if let Some(alias) = content.get("$alias").and_then(Json::as_str) {
            return self.vid(alias);
        }
        let content = if let Some(value) = content.get("$bundle") {
            let raw = ValueId::of_json(value).unwrap().to_string();
            self.made.insert(raw.clone(), value.clone());
            let mut dependencies = content["dependencies"].as_array().unwrap().clone();
            dependencies.sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
            dependencies.dedup();
            json!({"content":raw,"dependencies":dependencies})
        } else {
            content
        };
        let id = ValueId::of_json(&content).unwrap().to_string();
        self.made.insert(id.clone(), content);
        self.bind(ph, &id).unwrap_or_else(|e| panic!("{e}"));
        id
    }

    pub fn subst(&mut self, v: &Json) -> Json {
        match v {
            Json::String(s) if is_value_ph(s) => json!(self.vid(s)),
            Json::String(s) if kind_of(s).is_some() => {
                json!(self.fwd.get(s).unwrap_or_else(|| panic!("{s} 还没绑定")))
            }
            Json::Array(a) => {
                let mut out = Vec::new();
                for x in a {
                    out.push(self.subst(x));
                }
                Json::Array(out)
            }
            Json::Object(m) => {
                let mut out = serde_json::Map::new();
                for (k, x) in m {
                    let mut value = self.subst(x);
                    if k == "dependencies" {
                        if let Some(items) = value.as_array_mut() {
                            items.sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
                            items.dedup();
                        }
                    }
                    out.insert(k.clone(), value);
                }
                Json::Object(out)
            }
            other => other.clone(),
        }
    }

    pub fn content(&mut self, ph: &str) -> Json {
        if let Some(alias) = self
            .values
            .get(ph)
            .and_then(|v| v.get("$alias"))
            .and_then(Json::as_str)
            .map(str::to_owned)
        {
            return self.content(&alias);
        }
        let c = self
            .values
            .get(ph)
            .cloned()
            .unwrap_or_else(|| panic!("{ph} 要写在 values 里"));
        if let Some(alias) = c.get("$alias").and_then(Json::as_str) {
            return self.content(alias);
        }
        if let Some(bundle) = c.get("$bundle") {
            return self.subst(bundle);
        }
        self.subst(&c)
    }

    /// Definition 的业务内容；Bundle 本身由 vid 指认。
    pub fn definition(&mut self, ph: &str) -> Json {
        let _ = self.vid(ph);
        let bundle = self.made[&self.fwd[ph]].clone();
        self.made[bundle["content"].as_str().expect("Definition 是 Bundle")].clone()
    }

    /// 在绑定下逐字段比；失败只比 kind；不比 at。
    pub fn compare(&mut self, exp: &Json, act: &Json, path: &str) -> Result<(), String> {
        match (exp, act) {
            (Json::String(e), Json::String(a)) if is_ph(e) => {
                if is_value_ph(e) && !self.fwd.contains_key(e) && self.values.contains_key(e) {
                    let want = self.vid(e);
                    return if want == *a {
                        Ok(())
                    } else {
                        Err(format!("{path}：{e} 应为 {want}，实际是 {a}"))
                    };
                }
                self.bind(e, a).map_err(|m| format!("{path}：{m}"))
            }
            (Json::Object(x), Json::Object(y)) => {
                if is_failure(x) && is_failure(y) {
                    return self.compare(&x["kind"], &y["kind"], &format!("{path}/kind"));
                }
                let kx: BTreeSet<&String> = x.keys().filter(|k| *k != "at").collect();
                let ky: BTreeSet<&String> = y.keys().filter(|k| *k != "at").collect();
                if kx != ky {
                    return Err(format!("{path}：字段 {kx:?}，实际是 {ky:?}"));
                }
                for k in kx {
                    self.compare(&x[k], &y[k], &format!("{path}/{k}"))?;
                }
                Ok(())
            }
            (Json::Array(x), Json::Array(y)) => {
                if path.ends_with("/dependencies") {
                    let expected: Vec<_> = x.iter().map(|v| self.subst(v)).collect();
                    let a: BTreeSet<_> = expected
                        .iter()
                        .map(|v| v.as_str().expect("依赖 ID"))
                        .collect();
                    let b: BTreeSet<_> = y.iter().map(|v| v.as_str().expect("依赖 ID")).collect();
                    if a == b
                        && y.len() == b.len()
                        && y.windows(2).all(|w| w[0].as_str() < w[1].as_str())
                    {
                        return Ok(());
                    }
                    return Err(format!("{path}：依赖 {a:?}，实际 {y:?}"));
                }
                if x.len() != y.len() {
                    return Err(format!(
                        "{path}：{} 项，实际是 {} 项：{act}",
                        x.len(),
                        y.len()
                    ));
                }
                for (n, (e, a)) in x.iter().zip(y).enumerate() {
                    self.compare(e, a, &format!("{path}/{n}"))?;
                }
                Ok(())
            }
            _ if json_eq(exp, act) => Ok(()),
            _ => Err(format!("{path}：应为 {exp}，实际是 {act}")),
        }
    }
}
