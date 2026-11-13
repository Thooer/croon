//! Kernel 在宿主一侧接的效果契约与求值契约。实现前先读 crate 头部的五条义务。
//! 理由与 Kernel 侧的收尾规则：docs/Kernel/Reference.md（通道）；docs/Kernel/Language.md。

use crate::store::Content;
use serde_json::Value as Json;

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
    /// 幂等地封住这次物理 attempt 的写者与迟到启动。
    /// Kernel 提交之后、重启之前换 attempt、恢复时都会调用；同一 attempt 可能被退役多次。
    fn retire(&self, attempt: &str) -> Result<(), HostError>;
}
