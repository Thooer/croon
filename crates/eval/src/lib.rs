//! Croon 的形状与校验：Value、Schema、Definition 的形状，与 Language 的求值契约。
//!
//! 这里只有由形状推出的东西：内容怎样规范且寻址、Schema 怎样校验、Definition 的形状对不对、
//! Language 与 Kernel 之间一次会话的形状。它不知道 Journal、Reference、Control，也不知道
//! 任何提供者——那些都在 `croon` 与它之上。所以一份 Language 的实现只依赖这一层。

pub mod blocks;
pub mod definition;
pub mod host;
pub mod schema;
pub mod store;
pub mod value;

pub use store::Bundle;
