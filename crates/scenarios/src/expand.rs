//! 场景源码与目录的导入；只建立内容，不授予 Instance 资格。
//! 参考：docs/Kernel/Scenarios/README.md（格式）。

use kernel::host::{HostError, Read};
use kernel::store::{Bundle, ContentStore};
use kernel::value::ValueId;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default, Clone)]
pub struct Made {
    pub json: BTreeMap<String, Json>,
    pub bytes: BTreeMap<String, Vec<u8>>,
}
impl Made {
    pub fn put_json(&mut self, v: Json) -> String {
        let id = ValueId::of_json(&v).unwrap().to_string();
        self.json.insert(id.clone(), v);
        id
    }
    pub fn put_bytes(&mut self, bytes: Vec<u8>) -> String {
        let id = ValueId::of_bytes(&bytes).to_string();
        self.bytes.insert(id.clone(), bytes);
        id
    }
    fn pack(&mut self, content: String, mut dependencies: Vec<String>) -> String {
        dependencies.sort();
        dependencies.dedup();
        self.put_json(json!({"content":content,"dependencies":dependencies}))
    }
    pub fn import(&self, store: &ContentStore) {
        for value in self.json.values() {
            store.put_json(value).unwrap();
        }
        for value in self.bytes.values() {
            store.put_bytes(value).unwrap();
        }
    }
    fn get_bytes(&self, id: &str) -> Option<Vec<u8>> {
        self.bytes.get(id).cloned().or_else(|| {
            self.json
                .get(id)
                .map(|v| kernel::value::canonical(v).unwrap().into_bytes())
        })
    }
    fn get_bundle(&self, id: &str) -> Option<Bundle> {
        let v = self.json.get(id)?;
        Some(Bundle {
            content: v["content"].as_str()?.into(),
            dependencies: v["dependencies"]
                .as_array()?
                .iter()
                .map(|v| v.as_str().unwrap().into())
                .collect(),
        })
    }
}
impl Read for Made {
    fn code_bytes(&self, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.get_bytes(id))
    }
    fn code_json(&self, id: &str) -> Result<Option<Json>, HostError> {
        Ok(self.json.get(id).cloned())
    }
    fn code_bundle(&self, id: &str) -> Result<Option<Bundle>, HostError> {
        Ok(self.get_bundle(id))
    }
    fn bytes(&self, id: &str, _: &[String]) -> Result<Option<Vec<u8>>, HostError> {
        self.code_bytes(id)
    }
    fn json(&self, id: &str, _: &[String]) -> Result<Option<Json>, HostError> {
        self.code_json(id)
    }
    fn bundle(&self, id: &str, _: &[String]) -> Result<Option<Bundle>, HostError> {
        self.code_bundle(id)
    }
}

pub fn root() -> PathBuf {
    let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("CARGO_MANIFEST_DIR 必须存在");
    loop {
        if current.join("tracked.toml").is_file() {
            return current;
        }
        assert!(current.pop(), "仓库根要存在（向上找 tracked.toml）");
    }
}

pub fn expand(value: &Json, made: &mut Made) -> Json {
    match value {
        Json::Object(m) if m.len() == 1 && m.contains_key("$file") => {
            json!(std::fs::read_to_string(root().join(m["$file"].as_str().unwrap())).unwrap())
        }
        Json::Object(m) if m.len() == 1 && m.contains_key("$tree") => {
            json!(tree(&root().join(m["$tree"].as_str().unwrap()), made))
        }
        Json::Object(m) if m.len() == 1 && m.contains_key("$library") => json!(library(made)),
        Json::Object(m) if m.len() == 1 && m.contains_key("$bytes") => {
            let content = made.put_bytes(m["$bytes"].as_str().unwrap().as_bytes().to_vec());
            json!(made.pack(content, vec![]))
        }
        Json::Object(m) => Json::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), expand(v, made)))
                .collect(),
        ),
        Json::Array(a) => Json::Array(a.iter().map(|v| expand(v, made)).collect()),
        other => other.clone(),
    }
}
fn tree(path: &Path, made: &mut Made) -> String {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    let mut out = serde_json::Map::new();
    let mut dependencies = Vec::new();
    for e in entries {
        let name = e.file_name().into_string().unwrap();
        let kind = e.file_type().unwrap();
        let value = if kind.is_symlink() {
            json!({"link":std::fs::read_link(e.path()).unwrap().to_str().unwrap()})
        } else if kind.is_dir() {
            let id = tree(&e.path(), made);
            dependencies.push(id.clone());
            json!({"dir":id})
        } else {
            let content = made.put_bytes(std::fs::read(e.path()).unwrap());
            let id = made.pack(content, vec![]);
            dependencies.push(id.clone());
            json!({"file":id,"x":executable(&e.path())})
        };
        out.insert(name, value);
    }
    let content = made.put_json(Json::Object(out));
    made.pack(content, dependencies)
}
#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o111 != 0
}
#[cfg(not(unix))]
fn executable(_: &Path) -> bool {
    false
}

#[cfg(feature = "lang0")]
fn library(made: &mut Made) -> String {
    let id = tree(&root().join("croon-app/library"), made);
    let bundle = made.json[&id].clone();
    let mut content = made.json[bundle["content"].as_str().unwrap()].clone();
    let mut deps: Vec<String> = bundle["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    for (name, source) in generated_modules() {
        assert!(content.get(name).is_none());
        let bytes = made.put_bytes(source.into_bytes());
        let file = made.pack(bytes, vec![]);
        content[name] = json!({"file":file,"x":false});
        deps.push(file);
    }
    let raw = made.put_json(content);
    made.pack(raw, deps)
}
#[cfg(not(feature = "lang0"))]
fn library(_: &mut Made) -> String {
    panic!("$library requires lang0")
}

pub fn values(raw: &BTreeMap<String, Json>) -> (BTreeMap<String, Json>, Made) {
    let mut made = Made::default();
    let values = raw
        .iter()
        .map(|(k, v)| {
            let expanded = expand(v, &mut made);
            let value = if v.as_object().is_some_and(|m| {
                m.contains_key("$bytes") || m.contains_key("$tree") || m.contains_key("$library")
            }) {
                json!({"$alias":expanded})
            } else {
                expanded
            };
            (k.clone(), value)
        })
        .collect();
    (values, made)
}

/// Library 目录里由 spec 生成的三份模块；形状由本 crate 的 block 表给出，写法由 lang0 渲染。
#[cfg(feature = "lang0")]
pub fn generated_modules() -> [(&'static str, String); 3] {
    use lang0::library::{Decl, module};
    const PORTS: &[(&str, &str)] = &[
        ("pack", "Pack"),
        ("construct", "Construct"),
        ("instantiate", "Instantiate"),
        ("look", "Look"),
        ("notify", "Notify"),
        ("correct", "Correct"),
        ("revoke", "Revoke"),
    ];
    let kernel = crate::blocks::json("reference.kernel");
    let mut decls: Vec<Decl> = PORTS
        .iter()
        .map(|(port, name)| Decl {
            name,
            spec: &kernel[*port],
            schema: false,
        })
        .collect();
    let outcome = crate::blocks::json("definition.outcome");
    decls.push(Decl {
        name: "Outcome",
        spec: &outcome,
        schema: true,
    });
    let kernel_py = module(
        "\"\"\"Kernel 各 Port 的 Port spec 与 Outcome。由构建生成，不要手改。\"\"\"\nfrom croon import Port, Schema\n\n",
        &decls,
    );
    let model_py = module(
        "from croon import Port\n",
        &[Decl {
            name: "Generate",
            spec: &crate::blocks::json("model.generate"),
            schema: false,
        }],
    );
    let directory_py = module(
        "from croon import Port\n",
        &[Decl {
            name: "Exec",
            spec: &crate::blocks::json("directory.exec"),
            schema: false,
        }],
    );
    [
        ("kernel.py", kernel_py),
        ("model.py", model_py),
        ("directory.py", directory_py),
    ]
}
