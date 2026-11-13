//! 内容库：按 ID 分目录存 Value。
//! 参考：docs/Kernel/Value.md（实现）。
//!
//! 按内容寻址，写入是幂等的：同一个 ID 写几次都一样。Value 先落盘，引用它的记录之后才提交，
//! 所以没有记录引用的 Value 只是可以回收的垃圾，不影响正确性。

use croon_eval::host::HostError;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value as Json;

use croon_eval::value::{self, ValueError, ValueId};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{}：{source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error("{0} 是字节，不是 JSON Value 的规范文本")]
    NotJson(ValueId),
    #[error("{0} 的内容与它的 ID 对不上，库已损坏")]
    Corrupt(ValueId),
}

pub struct ContentStore {
    root: PathBuf,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_owned(),
        source,
    }
}

impl ContentStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<ContentStore, StoreError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(io(&root))?;
        Ok(ContentStore { root })
    }

    fn path_of(&self, id: &ValueId) -> PathBuf {
        let hex = id.hex();
        self.root.join(&hex[..2]).join(&hex[2..])
    }

    /// 存一个字节 Value，受 `raw_bytes` 限制。
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<ValueId, StoreError> {
        value::check_bytes(bytes)?;
        self.put_raw(bytes)
    }

    /// 存一个 JSON Value：存的是它的规范文本，受 `value.limits` 限制。
    pub fn put_json(&self, json: &Json) -> Result<ValueId, StoreError> {
        let text = value::canonical(json)?;
        self.put_raw(text.as_bytes())
    }

    fn put_raw(&self, bytes: &[u8]) -> Result<ValueId, StoreError> {
        let id = ValueId::of_bytes(bytes);
        let path = self.path_of(&id);
        if path.exists() {
            return Ok(id);
        }
        let dir = path.parent().expect("路径总有父目录");
        fs::create_dir_all(dir).map_err(io(dir))?;

        // 先写同目录下的临时文件并落盘，再改名：读到的要么没有，要么完整。
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let tmp = dir.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::File::create(&tmp).map_err(io(&tmp))?;
        file.write_all(bytes).map_err(io(&tmp))?;
        file.sync_all().map_err(io(&tmp))?;
        drop(file);
        fs::rename(&tmp, &path).map_err(io(&path))?;
        // 目录项也要落盘；只在 unix 上能这样打开目录，别的平台待定。
        #[cfg(unix)]
        fs::File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(io(dir))?;
        Ok(id)
    }

    pub fn contains(&self, id: &ValueId) -> bool {
        self.path_of(id).is_file()
    }

    /// 取出内容并核对哈希；没有这个 ID 返回 None。
    pub fn get_bytes(&self, id: &ValueId) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.path_of(id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(&path)(e)),
        };
        if ValueId::of_bytes(&bytes) != *id {
            return Err(StoreError::Corrupt(id.clone()));
        }
        Ok(Some(bytes))
    }

    /// 取出一个 JSON Value。内容必须正好是某个 JSON Value 的规范文本，否则它是字节，不是 JSON。
    pub fn get_json(&self, id: &ValueId) -> Result<Option<Json>, StoreError> {
        let Some(bytes) = self.get_bytes(id)? else {
            return Ok(None);
        };
        let not_json = || StoreError::NotJson(id.clone());
        let text = std::str::from_utf8(&bytes).map_err(|_| not_json())?;
        let json: Json = serde_json::from_str(text).map_err(|_| not_json())?;
        if value::canonical(&json)? != text {
            return Err(not_json());
        }
        Ok(Some(json))
    }
}

/// Explicit per-operation host protection; exhausting it does not invalidate content.
#[derive(Debug, Clone, Copy)]
pub struct Budgets {
    pub nodes: usize,
    pub read_bytes: usize,
    pub write_bytes: usize,
}

pub use croon_eval::store::Bundle;

#[derive(Default, Clone)]
pub struct Scope {
    pub values: BTreeSet<String>,
    pub bundles: BTreeSet<String>,
}

impl ContentStore {
    pub fn bundle(&self, id: &str) -> Result<Option<Bundle>, HostError> {
        let Some(id) = ValueId::parse(id) else {
            return Ok(None);
        };
        let Some(json) = self.get_json(&id).map_err(host)? else {
            return Ok(None);
        };
        Ok(croon_eval::store::parse(&json))
    }
}

pub struct Budget {
    nodes: Cell<usize>,
    reads: Cell<usize>,
    writes: Cell<usize>,
}
impl Budget {
    pub fn new(limits: Budgets) -> Rc<Self> {
        Rc::new(Self {
            nodes: Cell::new(limits.nodes),
            reads: Cell::new(limits.read_bytes),
            writes: Cell::new(limits.write_bytes),
        })
    }
}

/// Explicit DFS completion distinguishes shared dependencies from back edges.
fn walk_bundles(
    roots: Vec<String>,
    mut load: impl FnMut(&str) -> Result<Bundle, HostError>,
    mut complete: impl FnMut(String, Bundle),
) -> Result<(), HostError> {
    let mut stack: Vec<_> = roots.into_iter().map(|id| (id, None)).collect();
    let mut active = BTreeSet::new();
    let mut finished = BTreeSet::new();
    while let Some((id, bundle)) = stack.pop() {
        if let Some(bundle) = bundle {
            active.remove(&id);
            finished.insert(id.clone());
            complete(id, bundle);
            continue;
        }
        if finished.contains(&id) {
            continue;
        }
        if !active.insert(id.clone()) {
            return Err(HostError("cyclic Bundle dependencies".into()));
        }
        let bundle = load(&id)?;
        let dependencies = bundle.dependencies.clone();
        stack.push((id, Some(bundle)));
        stack.extend(dependencies.into_iter().map(|id| (id, None)));
    }
    Ok(())
}

pub(crate) fn host(e: impl std::fmt::Display) -> HostError {
    HostError(e.to_string())
}

/// Only explicitly delivered inputs and this execution's own writes are accessible.
pub struct Content {
    store: Rc<ContentStore>,
    scope: RefCell<Scope>,
    budget: Rc<Budget>,
}

fn charge(left: &Cell<usize>, amount: usize) -> Result<(), HostError> {
    let remaining = left
        .get()
        .checked_sub(amount)
        .ok_or_else(|| HostError("content budget exhausted".into()))?;
    left.set(remaining);
    Ok(())
}

impl Content {
    pub fn from_inputs(
        store: Rc<ContentStore>,
        roots: &[String],
        budgets: Budgets,
    ) -> Result<Self, HostError> {
        Self::new(
            store,
            Scope {
                values: BTreeSet::new(),
                bundles: roots.iter().cloned().collect(),
            },
            budgets,
        )
    }
    pub fn new(store: Rc<ContentStore>, scope: Scope, budgets: Budgets) -> Result<Self, HostError> {
        Self::with_budget(store, scope, Budget::new(budgets))
    }
    pub fn with_budget(
        store: Rc<ContentStore>,
        scope: Scope,
        budget: Rc<Budget>,
    ) -> Result<Self, HostError> {
        let this = Self {
            store,
            scope: RefCell::new(Scope {
                values: scope.values,
                bundles: BTreeSet::new(),
            }),
            budget,
        };
        this.expand(scope.bundles.into_iter().collect())?;
        Ok(this)
    }
    fn expand(&self, roots: Vec<String>) -> Result<(), HostError> {
        walk_bundles(
            roots,
            |id| {
                charge(&self.budget.nodes, 1)?;
                let parsed =
                    ValueId::parse(id).ok_or_else(|| HostError("invalid Bundle ID".into()))?;
                let bytes = self
                    .store
                    .get_bytes(&parsed)
                    .map_err(host)?
                    .ok_or_else(|| HostError("missing Bundle".into()))?;
                charge(&self.budget.reads, bytes.len())?;
                let json: Json = serde_json::from_slice(&bytes).map_err(host)?;
                if value::canonical(&json).map_err(host)?.as_bytes() != bytes {
                    return Err(HostError("noncanonical Bundle".into()));
                }
                let bundle = croon_eval::store::parse(&json)
                    .ok_or_else(|| HostError("invalid Bundle".into()))?;
                if !self
                    .store
                    .contains(&ValueId::parse(&bundle.content).unwrap())
                {
                    return Err(HostError("missing Bundle content".into()));
                }
                Ok(bundle)
            },
            |id, bundle| {
                let mut scope = self.scope.borrow_mut();
                scope.bundles.insert(id.clone());
                scope.values.insert(id);
                scope.values.insert(bundle.content);
            },
        )
    }

    pub fn bytes(&self, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        if !self.scope.borrow().values.contains(id) {
            return Ok(None);
        }
        let Some(id) = ValueId::parse(id) else {
            return Ok(None);
        };
        let bytes = self
            .store
            .get_bytes(&id)
            .map_err(host)?
            .ok_or_else(|| HostError("missing acquired content".into()))?;
        charge(&self.budget.reads, bytes.len())?;
        Ok(Some(bytes))
    }
    pub fn json(&self, id: &str) -> Result<Option<Json>, HostError> {
        let Some(bytes) = self.bytes(id)? else {
            return Ok(None);
        };
        let Ok(json) = serde_json::from_slice::<Json>(&bytes) else {
            return Ok(None);
        };
        if value::canonical(&json).map_err(host)?.as_bytes() != bytes {
            return Ok(None);
        }
        Ok(Some(json))
    }
    pub fn bundle(&self, id: &str) -> Result<Option<Bundle>, HostError> {
        if !self.scope.borrow().bundles.contains(id) {
            return Ok(None);
        }
        let Some(json) = self.json(id)? else {
            return Err(HostError("invalid acquired Bundle".into()));
        };
        croon_eval::store::parse(&json)
            .map(Some)
            .ok_or_else(|| HostError("invalid acquired Bundle".into()))
    }
    pub fn put_json(&self, json: &Json) -> Result<String, HostError> {
        let bytes = value::canonical(json).map_err(host)?;
        charge(&self.budget.writes, bytes.len())?;
        let id = self.store.put_json(json).map_err(host)?.to_string();
        self.scope.borrow_mut().values.insert(id.clone());
        Ok(id)
    }
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<String, HostError> {
        charge(&self.budget.writes, bytes.len())?;
        let id = self.store.put_bytes(bytes).map_err(host)?.to_string();
        self.scope.borrow_mut().values.insert(id.clone());
        Ok(id)
    }
    pub fn pack(&self, content: &str, dependencies: &[String]) -> Result<String, HostError> {
        if !self.scope.borrow().values.contains(content)
            || dependencies
                .iter()
                .any(|d| !self.scope.borrow().bundles.contains(d))
        {
            return Err(HostError(
                "pack references content outside this execution".into(),
            ));
        }
        let dependencies: BTreeSet<_> = dependencies.iter().collect();
        charge(&self.budget.nodes, 1)?;
        let id =
            self.put_json(&serde_json::json!({"content":content,"dependencies":dependencies}))?;
        self.scope.borrow_mut().bundles.insert(id.clone());
        Ok(id)
    }
    pub fn grants(&self, dependencies: &Json) -> bool {
        dependencies.as_array().is_some_and(|a| {
            a.iter().all(|d| {
                d.as_str()
                    .is_some_and(|d| self.scope.borrow().bundles.contains(d))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("croon-port-test-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    use serde_json::json;

    fn store(tag: &str) -> (TempDir, ContentStore) {
        let dir = TempDir::new(tag);
        let store = ContentStore::open(&dir.0).unwrap();
        (dir, store)
    }

    #[test]
    fn json_round_trips_through_its_canonical_text() {
        let (_dir, store) = store("json");
        let v = json!({"b": [1, 2.5, null], "a": "文"});
        let id = store.put_json(&v).unwrap();
        assert_eq!(store.get_json(&id).unwrap(), Some(v));
        assert_eq!(
            id,
            ValueId::of_json(&json!({"a": "文", "b": [1, 2.5, null]})).unwrap()
        );
    }

    #[test]
    fn writing_the_same_value_twice_is_the_same_and_harmless() {
        let (_dir, store) = store("idem");
        let a = store.put_bytes(b"hello").unwrap();
        let b = store.put_bytes(b"hello").unwrap();
        assert_eq!(a, b);
        assert_eq!(store.get_bytes(&a).unwrap().unwrap(), b"hello");
        assert!(store.contains(&a));
    }

    #[test]
    fn an_unknown_id_is_none_not_an_error() {
        let (_dir, store) = store("missing");
        let id = ValueId::of_bytes(b"never written");
        assert!(!store.contains(&id));
        assert_eq!(store.get_bytes(&id).unwrap(), None);
        assert_eq!(store.get_json(&id).unwrap(), None);
    }

    #[test]
    fn bytes_that_are_not_a_canonical_text_are_not_json() {
        let (_dir, store) = store("notjson");
        for raw in [
            &b"\xff\xfe"[..],
            b"not json",
            b"{ \"a\": 1 }",
            b"{\"a\":1,\"a\":2}",
            b"1.0",
        ] {
            let id = store.put_bytes(raw).unwrap();
            assert!(
                matches!(store.get_json(&id), Err(StoreError::NotJson(_))),
                "{raw:?}"
            );
        }
    }

    #[test]
    fn a_file_whose_bytes_are_canonical_json_is_that_json_value() {
        // JSON 与字节在同一个 ID 空间：文件内容恰好是规范文本，它就是那个 JSON Value。
        let (_dir, store) = store("space");
        let id = store.put_bytes(br#"{"a":1}"#).unwrap();
        assert_eq!(store.get_json(&id).unwrap(), Some(json!({"a": 1})));
        assert_eq!(id, store.put_json(&json!({"a": 1})).unwrap());
    }

    #[test]
    fn corruption_is_reported_not_returned() {
        let (dir, store) = store("corrupt");
        let id = store.put_bytes(b"original").unwrap();
        let hex = id.hex();
        fs::write(dir.0.join(&hex[..2]).join(&hex[2..]), b"tampered").unwrap();
        assert!(matches!(store.get_bytes(&id), Err(StoreError::Corrupt(_))));
    }

    #[test]
    fn values_outside_the_limits_are_refused_before_writing() {
        let (_dir, store) = store("limits");
        assert!(matches!(
            store.put_json(&json!(9007199254740993u64)),
            Err(StoreError::Value(ValueError::NumberTooLarge { .. }))
        ));
    }

    #[test]
    fn a_second_handle_on_the_same_directory_sees_earlier_writes() {
        // 重开 Kernel：内容库在磁盘上，不在进程里。
        let dir = TempDir::new("reopen");
        let id = ContentStore::open(&dir.0)
            .unwrap()
            .put_bytes(b"kept")
            .unwrap();
        assert_eq!(
            ContentStore::open(&dir.0)
                .unwrap()
                .get_bytes(&id)
                .unwrap()
                .unwrap(),
            b"kept"
        );
    }
    #[test]
    fn bundle_walk_distinguishes_shared_subgraphs_from_active_back_edges() {
        let mut loaded = Vec::new();
        let mut completed = Vec::new();
        walk_bundles(
            vec!["root".into()],
            |id| {
                loaded.push(id.to_owned());
                Ok(Bundle {
                    content: "value".into(),
                    dependencies: match id {
                        "root" => vec!["left".into(), "right".into()],
                        "left" | "right" => vec!["shared".into()],
                        _ => vec![],
                    },
                })
            },
            |id, _| completed.push(id),
        )
        .unwrap();
        assert_eq!(
            loaded.iter().filter(|id| id.as_str() == "shared").count(),
            1
        );
        assert_eq!(completed.last().unwrap(), "root");
        let error = walk_bundles(
            vec!["root".into()],
            |id| {
                Ok(Bundle {
                    content: "value".into(),
                    dependencies: vec![if id == "root" { "child" } else { "root" }.into()],
                })
            },
            |_, _| {},
        )
        .unwrap_err();
        assert!(error.0.contains("cyclic"));
    }
}
