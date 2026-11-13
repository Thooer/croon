//! Croon 的形状与校验：Value、Schema、Definition 的形状，与 Language 的求值契约。
//!
//! 这里只有由形状推出的东西：内容怎样规范且寻址、Schema 怎样校验、Definition 的形状对不对、
//! Language 与 Kernel 之间一次会话的形状。它不知道 Journal、Reference、Control，也不知道
//! 任何提供者——那些都在 `croon` 与它之上。所以一份 Language 的实现只依赖这一层。
//!
//! 实现一种 Language 之前必须知道的四条：
//!
//! - [`host::Language::check`] 在构造时跑：拒绝是业务失败（`CheckError::Rejected`），装载与 trap 是宿主故障。
//! - [`host::Session::resume`] 可以被反复调用，每次送回的只有**新到**的结果；它返回 Atom 或 done，
//!   不是最终值。返回同样的东西不代表可以重做已经报出的 Atom。
//! - `read` 只在这次调用里有效，不要留着。代码读取固定为 Definition 闭包，业务读取必须交出已消费的
//!   结果键；知道 ID 不授予资格。
//! - [`host::HostError`] 是宿主故障，不是 body 失败。它停下会话，不能由 body 接住。
//!
//! 求值契约的规则见 `docs/Kernel/Definition/Cognition.md`，插口见 `docs/Kernel/Language.md`。

pub mod blocks;
pub mod definition;
pub mod host;
pub mod schema;
pub mod store;
pub mod value;

pub use store::Bundle;
