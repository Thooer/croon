# Croon Kernel

Croon Kernel 是一个持久执行的制度机内核。它以主体为行动单位，以 Reference 表达权利，以 Journal 记录事实，以 Value 和 Bundle 交付不可变内容，以 Definition 描述主体的接口与做法。

Kernel 不预设智能体、模型或工具。模型、文件执行、Language 和 Library 都是接入 Kernel 的提供者或扩展。Kernel 只强制由自身保证推出的规则：行动可追溯、已发生的事不失真、Look/Notify/Correct 分离、授权只收窄。

## 内容

- `crates/eval/`：Value、Schema、Bundle、Definition 和求值契约的形状与校验。
- `crates/port/`：Port 与内容交付的契约；提供者只依赖这一层。
- `crates/tree/`：提供者共用的目录内容格式 Tree；Kernel 不依赖它。
- `crates/kernel/`：Instance、Journal、Reference、Control、恢复与提交。
- `crates/scenarios/`：不依赖具体 Language、模型或文件执行的 Kernel 场景与回归测试。
- `docs/`：Kernel 的设计、场景预期、实现选择与研究线索。

## 状态

设计验证与实现收敛中。当前重点是持久执行、显式 Bundle 交付、授权边界、恢复和 Kernel 与提供者的分离；Charter 已能实例化，Collective 与成员在一次事件里诞生；构造时「写死的 Definition 满足它的 Seat」、终止闭环与对 Collective 的 Correct 尚未实现，见 Definition 的「检查」与 Instance 的「待定」。

## 开发

本仓库是独立 Cargo workspace。Kernel 的构建与测试只使用本仓库的 crates 和依赖，不要求上层仓库参与。代码中使用的 JSON 形状以 block 为出处：在各 crate 的 `schema/<名字>.json` 下，由代码直接编译进来，文档按名字引用。

公开面是 `croon-eval`、`croon-port` 与 `croon-tree` 三个 crate；提供者与 Language 只依赖它们。`kernel` 是运行时，不是契约：装配系统的宿主依赖它，提供者不依赖它。每个 crate 的 `//!` 头部写着实现它之前必须知道的义务，那是那些义务的唯一出处。

在上层接入时按角色读，不必通读 docs：写宿主 Port（模型、文件执行等）读 `croon-port` 的头部，写 Language 读 `croon-eval` 的头部，写宿主（装起系统、驱动 root entry、处置宿主故障）读 `kernel` 的头部和它的最小宿主 `crates/kernel/tests/host.rs`。头部只写必须做什么，理由指回 docs 的具体小节；按角色的读法也列在 `docs/README.md` 的「地图」。读完这些还得去读实现才弄得清的行为，是文档的缺口，补进对应的头部或小节。

文档入口是 `docs/README.md`。系统设计在 `docs/Kernel/README.md`，Value、Definition、Instance、Journal、Reference 与 Port 分别从对应文件进入。

## 许可证

Apache License 2.0，全文见 LICENSE。
