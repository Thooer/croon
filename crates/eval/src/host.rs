//! Kernel 在宿主一侧接的东西之中，属于求值契约的那一半：Language 与它要的读取。
//! 参考：docs/Kernel/Definition/Cognition.md（求值契约）；docs/Kernel/Language.md（插口）。
//!
//! 另一半——Execution、Port、Channel、Call 与装配——在 `croon` 里：它们管效果怎样经通道收齐，
//! 由 Kernel 的保证推出，与求值契约无关。

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::Bundle;

/// Code reads use the fixed Definition closure; business reads name consumed result keys.
pub trait Read {
    fn code_bytes(&self, id: &str) -> Result<Option<Vec<u8>>, HostError>;
    fn code_json(&self, id: &str) -> Result<Option<Json>, HostError>;
    fn code_bundle(&self, id: &str) -> Result<Option<Bundle>, HostError>;
    fn bytes(&self, id: &str, consumed: &[String]) -> Result<Option<Vec<u8>>, HostError>;
    fn json(&self, id: &str, consumed: &[String]) -> Result<Option<Json>, HostError>;
    fn bundle(&self, id: &str, consumed: &[String]) -> Result<Option<Bundle>, HostError>;
}

/// 宿主保护或求值器实现故障；不是 body 的失败。
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct HostError(pub String);

#[derive(Debug, Clone, thiserror::Error)]
pub enum CheckError {
    #[error("{0}")]
    Rejected(String),
    #[error(transparent)]
    Host(#[from] HostError),
}

impl From<String> for CheckError {
    fn from(message: String) -> Self {
        Self::Rejected(message)
    }
}
impl From<&str> for CheckError {
    fn from(message: &str) -> Self {
        Self::Rejected(message.into())
    }
}

/// 一种 Language：按求值契约驱动的 body 求值器。
pub trait Language {
    /// 登记的名字加版本，如 `lang0@1`。
    fn id(&self) -> &str;
    /// 构造时的检查：body 与它用到的依赖代码合不合这种 Language。错误带路径。
    fn check(&self, definition: &Json, read: &dyn Read) -> Result<(), CheckError>;
    /// 开一次会话：`birth` 是 definition.eval 的 in 里的 birth，`results` 是已有结果的键 → 结果。
    fn open(
        &self,
        instance: &str,
        birth: &Json,
        results: &BTreeMap<String, Json>,
        read: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError>;
}

/// 一次会话。每次送回新到的结果（可以为空），得到 definition.eval 的 out：新报出的 Atom，或 done。
/// `read` 只在这次调用里有效，不要留着。
pub trait Session {
    fn resume(
        &mut self,
        results: &BTreeMap<String, Json>,
        read: &dyn Read,
    ) -> Result<Json, HostError>;
}
