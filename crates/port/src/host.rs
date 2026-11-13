//! Kernel 在宿主一侧接的效果契约与求值契约。
//! 参考：docs/Kernel/Language.md；docs/Kernel/Reference.md（通道）。

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
    /// Seal late starts and stop all writers for this physical attempt, idempotently.
    fn retire(&self, attempt: &str) -> Result<(), HostError>;
}
