//! Croon 的宿主 Port 与内容交付契约：提供者在这一层之上实现 Port、Execution 与 Language。
//!
//! 实现之前必须知道的五条。它们不在签名里，弄错会在运行中出事。
//!
//! - [`Port::channel`] 从 [`Channel`] 的两行里选一行。选 [`Channel::Isolated`]，上界必须由宿主
//!   实际强制；timeout、停止进程与产物收齐都不能依赖被执行程序自己配合。
//! - [`Port::retire`] 必须幂等。Kernel 在三处调用它：提交之后、重启之前换掉旧 attempt、恢复时
//!   按 Journal 收尾。同一个 attempt 可能被退役多次，也可能在 `start` 没走到时就要退役。
//! - [`Execution::collect`] 返回 `None` 表示还在跑，Kernel 用同一个键反复调用；返回 `Some` 之后
//!   不再调用。outcome 要满足 `definition.outcome`，它的 dependencies 必须落在本次交付的范围内。
//! - 返回 [`HostError`] 是宿主故障，不是 body 失败：Kernel 停下这次执行，Journal 不写业务结果。
//! - 不带超时的请求合法；Kernel 不为长请求设统一上限。
//!
//! 义务以上面这几条为准，docs 不复述。理由：通道与收尾见 `docs/Kernel/Reference.md`「通道」；
//! attempt 的接受、回收与重做见 `docs/Kernel/Instance.md`「执行」；端口表本身见 `docs/Kernel/Port.md`。

pub mod blocks;
pub mod host;
pub mod store;

pub use croon_eval::store::Bundle;
pub use host::{Call, Channel, Execution, HostDone, Port};
pub use host::{CheckError, HostError, Language, Read, Session};
pub use store::{Budget, Budgets, Content, ContentStore, Scope, StoreError};
