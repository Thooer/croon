//! Kernel 在宿主一侧接的东西之中，管效果的那一半：按来源装配的 Port、一次执行、时钟与故障点。
//! 求值契约那一半——Language、Session、Read——在 `croon-eval`，这里原样再导出，路径不变。
//! 参考：docs/Kernel/Language.md（插口）；docs/Kernel/Reference.md（通道）。

use serde_json::Value as Json;

#[cfg(feature = "runtime")]
use std::collections::BTreeMap;

#[cfg(feature = "runtime")]
use crate::store::Budgets;
use crate::store::Content;

pub use croon_eval::host::{CheckError, HostError, Language, Read, Session};

/// 接受时由可信通道固定；宿主类别本身不决定能否丢弃。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Discardable,
    Isolated,
}

pub struct Call<'a> {
    pub instance: &'a str,
    pub generation: u64,
    pub key: &'a str,
    pub source: &'a str,
    pub attempt: &'a str,
    pub value: &'a Json,
    pub dependencies: &'a [String],
    pub content: std::rc::Rc<Content>,
}

#[derive(Clone)]
pub struct HostDone {
    pub outcome: Json,
}

pub trait Execution {
    fn collect(&mut self) -> Result<Option<HostDone>, HostError>;
    fn discard(self: Box<Self>) {}
}

pub trait Port {
    fn id(&self) -> &str;
    fn channel(&self) -> Channel;
    fn start(&self, call: &Call) -> Result<Box<dyn Execution>, HostError>;
    /// Seal late starts and stop all writers for this physical attempt, idempotently.
    fn retire(&self, attempt: &str) -> Result<(), HostError>;
}

/// 故障点：脚手架在这里让 Kernel 进程「退出」。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "runtime")]
pub enum Fault {
    /// 一个事务提交之前；`records` 是它要写的 (Instance, 键)。
    BeforeCommit {
        records: Vec<(String, Option<String>)>,
    },
    /// 一个事务提交之后、更新索引之前。
    AfterCommit {
        records: Vec<(String, Option<String>)>,
    },
}

/// 脚手架在故障点上的判断。
#[cfg(feature = "runtime")]
pub type FaultHook = Box<dyn FnMut(&Fault) -> bool>;

#[cfg(feature = "runtime")]
pub struct Config {
    pub languages: Vec<Box<dyn Language>>,
    pub budgets: Budgets,
    /// 源 Handle → 实现。允许为空；根创建后也可通过 bind_host 装配。
    pub ports: BTreeMap<String, std::rc::Rc<dyn Port>>,
    /// 记录里的 `at`。
    pub clock: Box<dyn FnMut() -> String>,
    /// 返回真就在这里「退出」：Kernel 之后的调用都以 Crashed 失败，要重开。
    pub fault: Option<FaultHook>,
}
