//! 场景的脚手架：拿文档里写下的预期驱动真的 Kernel，跑完与预期的 Journal 逐条比对。
//! Kernel 的场景与 Language 的场景共用它；它不认识任何上层的形状，上层只经 [`drive::Setup`] 接进来。
//!
//! 写组合测试之前必须知道的四条。它们不在签名里，弄错测试会悄悄变弱，而不是报错。
//!
//! - [`drive::Setup::new`] 的 `root` 必须已经存在；场景里的 `$file`、`$tree` 都相对它解析。
//!   脚手架不知道兄弟仓库在哪，路径由调用方给，不要在调用方再复制一份 croon 的 root。
//! - [`drive::Setup::channels`] 按 host 名决定假提供者的通道。名字不在表里的 host 不会被绑定，
//!   也不报错：新增一个 host 就要同时登记它的通道。
//! - [`drive::Setup::provider_roles`] 决定真的 Language 的一次交出算不算交给提供者。没登记的按「不是」算，
//!   [`drive::Run::to_provider`] 返回 false，据它挑在途崩溃点的测试就会漏掉这次交出。
//! - [`drive::Setup::run`] 的 `seed` 只选一个兼容的全序；`at` 是 `(n, after)`：第 n 个事务处退出，
//!   n 从 1 数，`after` 为真表示提交之后。要枚举全部提交点，先跑一次 `at = None` 的基线，
//!   读它的 [`drive::Faults::commits`]。
//!
//! 场景里的 `sha256:…` 是占位名，运行时才绑定真实 ID，不要求是真哈希。
//! 参考：docs/Kernel/Scenarios/README.md（格式、脚手架、扰动）。

pub mod bind;
pub mod blocks;
pub mod drive;
pub mod expand;
pub mod fake;
pub mod scenario;
