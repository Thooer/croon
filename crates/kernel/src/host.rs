//! Kernel 的运行时配置与故障控制。
//! 参考：docs/Kernel/Journal.md（实现）；docs/Kernel/Reference.md。

use croon_port::Budgets;
use std::collections::BTreeMap;

pub use croon_port::{
    Call, Channel, CheckError, Execution, HostDone, HostError, Language, Port, Read, Session,
};

/// 故障点：脚手架在这里让 Kernel 进程「退出」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    BeforeCommit {
        records: Vec<(String, Option<String>)>,
    },
    AfterCommit {
        records: Vec<(String, Option<String>)>,
    },
}

/// 脚手架在故障点上的判断。
pub type FaultHook = Box<dyn FnMut(&Fault) -> bool>;

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
