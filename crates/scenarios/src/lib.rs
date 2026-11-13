//! 场景的脚手架：拿文档里写下的预期驱动真的 Kernel，跑完与预期的 Journal 逐条比对。
//! Kernel 的场景与 Language 的场景共用它。
//! 参考：docs/Kernel/Scenarios/README.md（格式、脚手架、扰动）。

pub mod bind;
pub mod blocks;
pub mod drive;
pub mod expand;
pub mod fake;
pub mod scenario;
