# 文档

Croon 是什么、讨论与写作的规则，见仓库根的 README 与 AGENTS。本文件只说 Kernel 这一层：系统由哪些部分构成、依赖什么、引用往哪个方向，以及想问什么去哪里读。

本仓库是三个仓库里最低的一层，只含 Kernel 与它对外公布的契约。Language 的求值器在 croon-lang0，目录、模型、Library 与 Application 在 croon-app。它们依赖本仓库，本仓库不认识它们的任何形状。

## 骨架

三样基本元素：

- **Value**：不可变，按内容寻址。给出一个 Value，只给出信息。
- **Instance**：可变状态的主人，每次变化由它自己的 Journal 解释。
- **Reference**：唯一的权利，由 Kernel、别的 Instance 或外部服务提供。

Definition、Bundle、Schema 与 Port spec 是 Value 的形状；Journal 保存 Instance 的事实。Language 的求值器与 Plugin 由根装；Directory、Model 与 Library 在 Kernel 之外，是提供者一侧的领域规格。通用 Kernel 与智能体用例的边界见 Kernel/README.md 的「制度机」。

| 部分 | 承担 | 依赖 |
|---|---|---|
| Value | 内容寻址、Schema、Bundle | 无 |
| Definition | Interface 与 Implementation：Signature、Port、附带；求值契约；Charter 与 Seat | Value、Bundle |
| Instance | 身份与 Journal、重放、同一性、来处、执行、Control；Collective 的产生与结束 | Value、Definition |
| Reference | 持有、Handle、接上、附带、撤销、收窄、通道；Kernel 的 Port | Value、Definition、Instance |
| Language | body 求值器的插口：契约、ID 与版本、守什么 | Definition、Instance |
| Plugin | 宿主代码：实现 Language，或提供现有 Port 组合不出来的能力；由根装，可信由根承担 | Definition、Reference |

各个部分各自成文，从部分的名字就能找到各自的文档。依赖方向为：Definition 与 Reference 依赖 Value 和 Bundle；Language 依赖内容交付和 Port 契约。Kernel 不解释 Directory 或 Model 的领域结构。

这一层之上还有提供者一侧的领域规格（Directory、Model）与组合（Library），以及具体的 Language。它们各自成仓库，依赖本层公开的契约，本层的文档不解释它们。

## 分类与关系

主体是能行动并维持状态连续性的单位，可以是人、程序或它们组成的整体。系统内的主体以 Instance 存在，分类按行动由谁承担：

| 组成 | 行动与权利 | Definition 的做法 |
|---|---|---|
| Individual | 自己报出 Atom，持有一份 c-list，行动记在自己的 Journal | Cognition 固定 body，由宿主中的 Language 求值；外部驱动时称 Entry，Definition 只固定 Interface |
| Collective | 行动由成员中的 Individual 承担，各身份有自己的 Journal；整体没有自己的一步 | Charter 固定 Seat、接线与导出，由 Kernel 解释结构；成员可再是 Collective |

Individual 与 Collective 区分身份怎样组成；Entry 区分 Individual 的执行安排。三种 Definition 形状因此对应两种组成形式。两种组成形式共用 Interface，行为与分权分别由什么机制兑现，见 Kernel/Definition/README.md 的「Cognition 与 Charter」；Entry 的执行、暂停与重连见 Kernel/Definition/Cognition.md 的「求值器在系统外」。

Cognition 中的模型、算法和上下文怎样组合，由 Definition 的作者设计、Principal 选用；Entry 的做法由系统外客户端决定。固定程序可以包含推理，使用模型也可以只完成固定流程。Kernel 检查的是求值契约、Interface 与持有的 Reference，智能与否不构成分类依据。

关系中的称谓与上述分类分别成立：

| 关系 | 称谓 | 说明 |
|---|---|---|
| 委托 | Principal / Agent | 委托产生 Agent，委托归 Principal；成员发起的委托归所属的 Collective，规则见 Kernel/Instance.md 的「委托」 |
| 组成 | 成员 / Collective | 成员占着 Seat，随所属 Collective 产生与结束 |
| 系统边界 | 根 / root entry | 根在系统外造成 root entry，之后经它在系统内行动；其余 Entry 按普通接线取得权利，接入规则见 Kernel/README.md 的「待定」 |

P 委托 Individual A，A 使用两套上下文、两个模型，又委托 B。A 同时是 P 的 Agent 与 B 的 Principal，模型与上下文共用 A 的权利；需要不同权利边界时，分别实例化并接线。保管者、登记者、熔断者、转交者等名字说明组合中的职责，定义在各自的用法里。

## 地图

想问什么，去哪里读。每一行的第一列是问题，不是文件名。收什么内容才能算这一部分，见仓库根 AGENTS 的「位置」：那张表管写到哪里，这张表管从哪里读。

| 想问什么 | 读哪里 |
|---|---|
| Croon 是什么、现在做到哪了 | 仓库根的 README.md 的「现在」；未实现的部分在各部分的「待定」 |
| 系统由哪些部分构成、依赖什么 | 本文的「骨架」；上层各部分的文档在各自的仓库 |
| 一个概念在正文里叫什么，几种名称是什么关系 | 本文的「名词表」与「分类与关系」 |
| 制度机、零环境特权、测试与生态边界是什么 | Kernel/README.md 的「制度机」 |
| Kernel 保证什么、一样机制该放在哪 | Kernel/README.md 的「保证」「放置」；同一文件里的「前提」写作法要照顾的引擎特性 |
| 某一种能力组合得出来吗 | Kernel/README.md 的「运作方式」与「场景」 |
| 根、Entry、系统边界是什么 | Kernel/README.md 的「根」与「待定」 |
| Instance、Journal、记录、恢复 | Kernel/Instance.md、Kernel/Journal.md |
| 权利怎样给、收窄、撤销；通道 | Kernel/Reference.md；Kernel 提供的 Port 的形状在 Kernel/Port.md |
| Definition 怎样写、Signature、Port、附带、Charter | Kernel/Definition/README.md；形状与构造时的检查在 Shape.md |
| 求值契约、Atom、失败、求值器在系统外 | Kernel/Definition/Cognition.md；会话的形状在 Session.md |
| Language 是什么、要守什么 | Kernel/Language.md |
| 宿主代码、可信由谁承担 | Kernel/Plugin.md |
| 跨部分的实现选择与理由 | Kernel/Engineering.md；绑某一部分的在那一部分的「实现」一节 |
| 某个场景应当跑出什么 Journal | Kernel/Scenarios/；格式、脚手架与扰动见那里的 README.md；内容交付场景使用 Bundle |
| 我在写一个宿主 Port（模型、文件执行等），要守什么 | `croon-port` 的 crate 头部；Kernel/Reference.md 的「通道」；Kernel/Instance.md 的「宿主故障与失败」 |
| 我在写一种 Language，要守什么 | `croon-eval` 的 crate 头部；Kernel/Definition/Cognition.md 的「求值契约」；Kernel/Language.md |
| 我在写宿主：怎样装起一个系统、驱动 root entry、处置宿主故障 | `kernel` 的 crate 头部，最小的宿主见 crates/kernel/tests/host.rs；Kernel/Port.md 的「根绑定」；Kernel/Instance.md 的「执行」与「宿主故障与失败」 |
| 证据、先例、借鉴的判据 | Research/；那里的内容待核验，只作线索 |

回归集是 Kernel/README.md 的场景表：删改 Kernel 之后重新走一遍。子文件夹的 README.md 只写这一部分讲什么，不再列读法。

跨目录的约定：代码使用的 JSON 形状只在 block 里定义（在 `crates/*/schema/<名字>.json`），概念的名字只在下面的名词表里。

## 名词表

一个概念一行，按部分排。「正文」是正文与讨论里专指它时用英文名还是中文名；规则见仓库根 AGENTS 的「用词」。本表只收 Kernel 自己的词；Language、Directory、Model 与 Library 的词在各自仓库的名词表里，这里不重新定义。

| English | 中文 | 正文 | 用法 |
|---|---|---|---|
| Cognizer | 主体 | 中 | 系统内的分类见「分类与关系」 |
| Kernel | 内核 | 英 | |
| Institution Machine | 制度机 | 中 | Kernel 执行规则、权利与事实所承担的抽象，见 Kernel/README.md |
| root | 根 | 中 | 只指系统外、造成诞生的一方，记录里写作 outside |
| Entry | 入口 | 英 | 外部驱动的 Individual；根的入口写 root entry |
| Value | 值 | 英 | 内容寻址的 JSON、字节与 Bundle |
| Bundle | 内容包 | 英 | 固定内容与显式依赖的不可变整体，见 Kernel/Value.md |
| Definition | 定义 | 英 | |
| Interface / Implementation | 接口 / 做法 | 英 | Implementation 只指 Definition 的 Implementation |
| Signature | 签名 | 英 | |
| Port；Provided port / Required port | 端口；服务端口 / 需要端口 | 英 | JSON 里的 port 只指 Port 的名字 |
| Port spec | Port 的说明 | 英 | JSON 里的 spec 是它的 ID；description 写描述 |
| Attachment | 附带 | 中 | |
| Control；Look / Notify / Correct | 控制；看 / 结束通知 / 纠正 | 英 | Control 是三者的全体或其中几个，只说 Correct 就写 Correct。Look 有查看与读取，Correct 有暂停、解除暂停、授予、收回；「恢复」只指崩溃后的恢复 |
| Competence | 能力 | 英 | |
| Cognition / Charter | 认知 / 章程 | 英 | |
| Seat | 席位 | 英 | |
| Individual / Collective | 个体 / 集体 | 英 | |
| Atom | 原子 | 英 | 结果不能重算的一步 |
| Send / Receive | 交出 / 取 | 中 | 单用的「取」只指 Receive，日常的意思写成选、用、拿；取决、读取这类词不算 |
| Session | 会话 | 中 | 只指 Kernel 与求值器之间的会话 |
| Instance | 实例 | 英 | |
| Journal | 实录 | 英 | |
| Birth / Arrival | 诞生 / 到达 | 中 | 「到达」只指 Arrival，经 Port 够得着写够到 |
| Event | 事件 | 中 | |
| origin | 来处 | 中 | Instance 从哪里来：委托或成员，沿它往上到根 |
| Principal / Agent | 委托者 / 受托者 | 英 | 用于委托关系，见「分类与关系」 |
| Handoff | 交接 | 英 | |
| executor | 执行者 | 中 | 只指 Look、做、Correct 三种角色里做事的一方 |
| Reference | 引用 | 英 | |
| source | 来源 | 中 | 只指权利从哪里来 |
| route | 来路 | 中 | 一项到达交到哪个 Port、谁交的、沿哪一份来 |
| provenance | 出处 | 中 | 信息与文献从哪里来 |
| Handle | 句柄 | 英 | |
| Revoker | 撤销权 | 中 | |
| Language | 语言 | 英 | |
| Ref | 引用值 | 中 | Language 里指向一个 Reference 的值；Ref 与 Reference 太近，所以用中文 |
| Plugin | 插件 | 英 | |
| Custodian | 保管者 | 中 | 见 Kernel/Reference.md 的「持有意味着什么」 |
| Registrar | 登记者 | 中 | 同上 |

参考：Kernel/README.md（制度机、根、待定）；Kernel/Definition/README.md（Cognition 与 Charter）；Kernel/Definition/Cognition.md（求值器在系统外）；Kernel/Instance.md（委托）。
