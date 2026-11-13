//! Croon 的宿主 Port 与内容交付契约。
//! 参考：docs/Kernel/Port.md；docs/Kernel/Definition/Cognition.md。

pub mod blocks;
pub mod host;
pub mod store;

pub use croon_eval::store::Bundle;
pub use host::{Call, Channel, Execution, HostDone, Port};
pub use host::{CheckError, HostError, Language, Read, Session};
pub use store::{Budget, Budgets, Content, ContentStore, Scope, StoreError};
