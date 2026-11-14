# Kernel

Croon 是什么、为什么做，见仓库根的 README.md；系统总览与引用方向见 ../README.md。本目录是 Kernel 的设计：本文讲系统。文档力求不说一句废话。

## 制度机

Kernel 执行的是有根系统内的规则、权利与事实：Definition 规定行动与组织结构，Reference 决定能造成什么效果，Journal 记录已经发生的事，Control 把纠正与执行分开。不可变内容由 Bundle 交付，目录、模型与执行环境由提供者解释。制度机是这些机制共同承担的抽象，机制与依赖仍按 ../README.md 的「骨架」划分。

智能体是 Kernel 的一个特定用例。更换为审批、协作或其他用途，只更换 Definition、Language、Port 的提供者与 Library，四条保证保持成立。这个通用性由下面的无模型审批验收：不装 llm/exec 的 Approval 运行与恢复通过。

模型的分类不是 Kernel 抽象的条件。模型是当下最常见的求值器与提供者，但不是唯一的一种：一个只算数的求值器、一个人工驱动的 Entry、一个不装任何外部服务的审批程序，在 Kernel 里都是同一种东西——一个持有 Reference、报出 Atom、记在 Journal 里的主体。所以机制里不出现模型、提示词与推理的概念，它们属于选定那种做法的一侧；需要跨边界时写成 Port 的 Port spec，不进入 Kernel 的 Port 表。智能与否、是否用模型，不构成分类依据，见 ../README.md 的「分类与关系」。

零环境特权贯穿所有用途：调用模型与工具、运行 Library 和预置 Application 的 Instance，都仅凭持有的 Reference 造成效果，预装不额外授予权利；来源与接线见「根」和 Reference。核心测试检验四条保证、通道与求值契约；具体 Language、提供者和 Application 检验各自语义与组合，跨边界场景检验接合。生态通过这些边界扩展，预置与第三方遵守同一规则；通用 Kernel 与智能体部分的实现分离由开发规划落实。

Kernel 强制的是系统内经通道发生的行动。什么算有效意见、怎样算通过，由审批的 Definition 规定；Kernel 检查实际持有的权利、Interface 的形状与执行契约。具体做法能否兑现功能，由选用它的 Principal 判断。根仍承担宿主、提供者与系统边界上的可信；Journal 只追加不等于抵抗宿主篡改，跨信任边界的防篡改仍是 Journal.md 的待定。Language 的系统内构造、求值器隔离与制度机通用性分别验收，元循环完成与否不决定其他用途能否成立。

## 主体

主体的分类与关系见 ../README.md。Definition 承载功能，不承载意志：功能是构造它的一方的动机，Interface 承诺做什么，由 Instance 兑现；意志是 Principal 选用这份承诺要完成的事。Principal 本身也是主体，意志借自己的功能实现，沿来处往上，最上方是根的意志。功能能否做到、判断是否偏离意图，靠具体做法与检验；需要强制的分权，由不同 Instance 之间的 Reference 与 Control 承担，见 Definition/README.md 的「Cognition 与 Charter」。

LLM 是引擎，不是主体：它本身不提供跨调用的连续性，连续性由框架在引擎之外提供。目标是功能等价，不求机制等价：在工作所需的能力上做到人能做到的，做法可以不同。

## 前提

本节是智能体用例对引擎的工作假设，适用条件因模型与 provider 而异；它指导 Language 与 Library，不增加 Kernel 机制。写法与结构化程度这类做法靠实验定：

| 特性 | 设计上的后果 |
|---|---|
| 上下文有限，关键信息埋得越深越容易被忽略 | 主动管理上下文；Principal 不携带 Agent 的中间步骤 |
| 精准紧凑的组织有助于处理更复杂的问题 | 用结构而不是长文本协调工作 |
| 重复的前缀可以复用计算 | 稳定的内容放在前缀，变化的内容追加在后 |
| 每次调用都要重新提供全部输入，目标会被表面线索带偏 | 目标与约束由结构每次重新提供 |
| 一次调用中途打断会损害推理 | 只在调用之间介入 |
| 同一件事反复更新时，早先的值压制后来的 | 上下文只放当前值 |
| 没有自己的停止与检验，报告的把握不会自动用来调节自己 | 停止与检验由结构决定，检验独立于生成 |
| 输出不因读者是谁而改变 | 跨边界传递整理过的结果 |
| 同一模型的实例犯相似的错，越准确的模型错误越相似 | 独立性靠异质取得，多开实例取得不了 |
| 没见过的语法，模型会照着相似的语法去猜 | least astonishment（最小意外）：与模型的先验每差一处，就要靠提示或报错补一处，所以差得越少越好；写法像、语义却悄悄不同的，不报错，补不上，所以不允许。Language 借用模型熟悉的写法，不自造关键字；构造时检查兜底，报错说出违反了哪条规则 |
| 跨语言的泛化不对称，随任务与模型而异；用哪种语言会改变推理的路径、token 的效率与激活的领域知识 | 面向模型的文字（提示、工具说明、构造报错）以适合任务的语言为主要部分，关键概念保留原文 |
| 否定的指令不如肯定的可靠 | 提示与报错说做什么；要划边界，就写成「用 X 代替 Y」 |
| 严格的格式约束会挤占推理 | 程序逻辑先写，再检查、修改；不依赖约束解码 |

约束的内容会变，方法不变：引擎变强之后，隔离与协调仍然需要，理由会从省上下文变成并行、探索多条路径。

## 保证

| 保证 | 机制 | 部分 |
|---|---|---|
| 行动可追溯：谁做的、用什么、凭什么 | 每个 Atom 记在执行它的 Instance 的 Journal 里；跨 Journal 的事件有 ID，两边都记；沿来处往上只有一条路，到根为止；能动什么由持有的 Reference 决定 | Instance、Reference |
| 已发生的事不失真 | Journal 只追加；Value 不可变；可变状态的每次变化都由一本 Journal 按顺序解释，它所属的 Instance 是主人；当前状态都是投影 | Value、Instance |
| 修正权独立：Look、做、Correct 分开授予 | Control 是 Look、Notify、Correct 三个 Port，交给实例化的一方，可以分开转交，Charter 只把收窄过的几份接给成员；由 Kernel 执行，目标拒绝不了；Kernel 拒绝让执行者持有指向自己的 Correct；成员之间的接线只按 Charter 写明的方式改变，上层也改不了 | Instance、Reference |
| 授权只收窄 | Reference 只能来自已持有的 Reference；每一节收窄留在派生链上，每次交出都合取；模型、提示词、生成的代码都不是授权来源 | Reference |

## 根

每个 Instance 只有一个来处，沿来处往上只有一条路，到根为止。权利只能来自已持有的 Reference，所以没有来源的 Reference（账号、新的 provider、Plugin 提供的）只能从根进来，责任链最后落到根。根是系统外、造成诞生的那一方，记录里写作 outside。根在人：代码与制度都是人写的，来处的最上方是人；人在这里是一个主体，没有超过主体的权利。

根在系统里经 root entry 行动，Entry 的执行契约见 Definition/Cognition.md 的「求值器在系统外」。理由：Reference 只在 Instance 的 c-list 里，系统外的一方持有不了；根若直接在别的 Instance 那里造成记录，这次事件就没有一本 Journal 记着造成它的那一步，根拿到的 Handle 也没有 Journal 解释。由此：

- root entry 是唯一由系统外造成诞生的 Instance，在来处的最上方，再往上就是根。没有来源的 Reference 在它诞生时接到它的 Required port 上，各在一条派生链的最上方；它的 Required port 也可以接到自己，写在诞生的 self 里，形状同 instantiate 的 self。别的 Instance 的 self 写在实例化那次交出的 Value 里，推得出，诞生里不另记。对它的 Look、Notify 与 Correct 不给任何一方：系统外没有 c-list，停下它是根在宿主上的事。
- 根在系统里做的事都是 root entry 的 Atom：构造、实例化、交出、取、读取。拿到的 Port 与 Control 在它的 c-list 里，记在它的 Journal 里。
- 根在系统外做的事不进任何 Journal：配置凭据，装 Language 与 Plugin，连同接线与初始输入创建 root entry，在宿主上看全部 Journal。

没有采用让一个内部程序取到根交来的命令再照做：每条命令记两次，每条都由系统外造成；body 等于把 API 再写一遍；回复还要一条出去的通道。

系统是一个 Kernel 管着的一组 Journal，其中的事件语义对得上：交出恰好一次，撤销立即生效，交给已结束的 Instance 被拒。一个系统只有一个根。系统可以连到别的系统：对本系统来说，远程的系统是一个外部服务，或是驱动 Entry 的一方，也就是本系统的根。系统之间连成网，不需要全局的根。

隔离有两层：

- **同一个根下的 Instance 之间，靠 Reference**：授权只收窄，能撤销。泄露一个 Reference，损失不超出这个根持有的东西，责任也还在这个根。
- **不同的根之间，靠系统边界**：Handle 只在自己系统的 c-list 里解析，对方只是外部服务，一个 Reference 泄露不到别的系统里去。所以不同的根就是不同的系统，事务也以系统为单位，见 Journal 的「记录」。

对一个 Instance 与它的后代来说，实例化它的一方就像根，这是相对的：授权只有这一个来源；它们像在一个完整的世界里，上面是谁，不需要知道。

## 放置

一样机制放在哪部分，看它从哪里来：

- 由保证推出的事实与不变量（Journal、内容寻址、Reference 的检查、fencing、求值契约）在 Kernel。
- 由需要决定的（认知模式、预置的 Charter、上下文管理、历史视图）写在 Library。
- 拿不准时先不进 Kernel：加入是增量，移出会破坏依赖它的扩展。
- Kernel 提供机制，不预设策略（separation of mechanism and policy，机制与策略分离）：两种 Kernel 设计都可行时，选让扩展能组合出更多做法的那个。这和闭合的"最小"不冲突：最小是去掉多余的机制，这一条是让每个机制不偏向某一种做法。由此：
  - Kernel 不预设两端的信任。收什么、怎么对待、给出什么，由每一端自己的 Definition 决定；Kernel 只管权利。
  - 信息与 Reference 暴露给谁、暴露到什么粒度、能不能被猜到，由暴露的一方设计并负责；Kernel 只保证没暴露的拿不到：权利只看 c-list，Value 只给出信息，存储不是后门。Reference 给出去还能撤销，信息给出去就拿不回，所以暴露之前就要想好。一个主体在已有权利内交出私有数据，责任在把数据放到它够得着的一方，即做了这个组合的 Principal。
  - 聊天、消息、收件箱是呈现与用法，不进 Kernel。Kernel 里交出的只是 Value 与 Reference。
  - Kernel 不预设 Control 往下够到多深。穿透与封闭、有条件的介入，由 Principal 组合；Kernel 保证组合按写的执行，能拿到的 Reference 的上限运行前看得见。
- 新增一个 Kernel 的 Port 或一种 Language 插口机制之前，先在运作方式表里写出一行现有组合做不到的。当前 Language 由根登记；让求值器在系统内构造的缺口与候选见 Language.md 的待定。
- 组合不出来的能力做成 Plugin，由根装，见 Plugin。
- 可信的含义与承担者见 Plugin 的「可信」。

## 运作方式

主体的运作方式都由骨架组合而成。这张表用功能检验 Kernel 与扩展，标准有两条：完备，每一行都组合得出；最小，Kernel 里的每样东西都有一行用到它，或有一条保证要它，而且这一行或这一条别的东西承担不了。两条都成立就是闭合，删改 Kernel 之后要重新成立。组合不出来的，标出缺口，缺口要进 Kernel，必须由保证推出。

表里只列能力，「承担」只写哪几部分的哪样机制；组合的写法在各部分。下游用什么填这些位置不属于本仓库：单次调用与固定工作流由一个在 Kernel 之外实现、经 Language 插口接进来的求值器完成；认知模式由若干 Definition 按 Charter 组合；工具是接上去的 Reference。具体的需要与做法写成场景，见下一节，每个场景标出它检验哪几行。

| 运作方式 | 承担 |
|---|---|
| 单次调用、固定工作流 | Cognition 的 body，由外接的求值器经 Language 插口执行 |
| 认知模式、反复行动 | 若干 Definition 按 Charter 与接线组合，行为都在成员里 |
| 作用于外界：文件、命令、外部服务 | Reference（Kernel 的 Port、通道）、Bundle（显式输入与产物）、工具（接上的 Reference） |
| 累积 | Instance（Journal）、Value |
| 维持目标与约束 | Lang0（每次调用由结构重新提供） |
| 抑制与遗忘 | Library（上下文管理） |
| 出处 | Instance（Journal）、Library（呈现） |
| 停止 | Lang0（循环上限、按检验结果走不同的路） |
| 程序化 | Definition（构造）、Instance（接替） |
| 独立检验 | Instance（Look 与 Correct 分开）、Reference（收窄跟着 Reference 走）、Definition（Charter 的 pin 与接线）、Library（预置 Charter、检查器） |
| 打断 | Instance（交到 Provided port、暂停）、Lang0（取）、Library（呈现给模型） |
| 委托：等结果、不等结果、对话 | Instance（实例化、Notify、Control 的转交）、Definition（附带的嵌套）、Lang0（取、交出到运行中拿到的引用值）、Library |
| 交接 | Instance（Handoff、同一性）、Library（转交者、派单者） |
| 换接上的东西 | Instance（Correct 的授予与收回） |
| 撤销转交出去的权利 | Reference（撤销权） |
| 只读观察 | Instance（Look、Notify）、Library（投影） |
| Collective | Definition（Charter、Seat）、Instance（产生与结束、来处、Control）、Library（预置 Charter） |
| 共享与保密 | Definition（body 的边界）、Library |
| 插入 | Library（接保管者、Middleware、交到 Provided port、工具目录） |

## 场景

场景只写谁想做什么，不写结论，用具体的需要检验 Kernel 与扩展。走查过的场景成表：场景、做法、检验的运作方式，最后一列是本文运作方式表的行。表一直保留，是回归集：删改 Kernel 或 Library 之后，重新走一遍。运作方式表里每一行至少有一个走查过的场景，完备才有证据；走查中发现 Kernel 的缺口，在运作方式表里标出。

| 场景 | 做法 | 运作方式 |
|---|---|---|
| 熔断者暂停执行者 A，A 早先交办出去的 B 照跑 | 默认够不到 B。要够到，A 的 instantiate 接到登记者，熔断者持有登记者暂停 A 所产生的 Instance 的 Port | 独立检验、委托 |
| 公司的采购成员外包给供应商，之后采购被说动、被暂停 | Charter 写死登记者，C 的 instantiate 只接给它；供应商的 Control 在登记者手里，收窄的几份交回采购；C 的 Principal 经登记者导出的 Port 按规则介入 | Collective、委托 |
| 上层被说动，想悄悄撤掉审计者对执行者的读取 | 带 Seat 路径的只有 Look 与 Notify，Correct 的 in 里没有路径，撤不掉；上层只能暂停整个 Collective，或换一个新的 Collective，结束记在每个成员的 Journal 里 | 独立检验、Collective |
| 评估者持有隐藏的评分材料，执行者想拿到，或猜出低熵的答案再算哈希确认 | 评估者是保管者，执行者不持有它的 Port；自己算出的 ID 不在它的 Bundle 取得范围里；普通内容只能给出字节，不会展开 Bundle；另一个同样读不到的 Instance 把字符串交给它，Kernel 不传 | 共享与保密 |
| Agent 把产物 Bundle 写进 Signature 的 out 交回，Principal 比较基准并采用或合并；审计者取得明确的 Bundle | Agent 能读，根随结果交给 Principal；Look 只传递所选记录的实际交付集合 | 委托、独立检验 |
| B 交来消息并附带回话途径，内容说动 ReAct 要把 exec 附带过去 | 消息与回话途径在同一条记录里，给模型渲染成名字；模型只能在 petname 表里选；exec 是 Required port，不能按数据找到，模型写出 "exec" 也解析不到；ReAct 写的回话途径不收附带，Kernel 按它自己那一节拒绝 | 委托、插入 |
| 登记者交回实例化出的 Instance 的 Correct，只想给暂停 | 登记者的请求 Port 在回话途径的 Port spec 里写明 pause 只收暂停；派生时接上这一节，请求者拿到的只能暂停。收窄写在写死的 Definition 里，构造时看得见 | Collective、独立检验 |
| 熔断者把只能暂停的那份附带给一个声明了完整 Correct 的外人 | Seat 写的「只收 pause」那一节留在派生链上，外人也只能暂停；resume 以 denied 被拒 | 独立检验、撤销转交出去的权利 |
| 招募者运行中招来 N 个投票者，交给聚合者 | 聚合者的 Port 的附带声明 additionalProperties 是投票的 Port spec；名字任意，每份只能当投票用，上限算得出 | Collective |
| Principal 同时等几个 Agent，选最先结束的，其余不要了 | 各向 Notify 登记一次，to 是同一个指向自己的 Reference，得到各自的 Handle；按 via 认出是谁；暂停其余的，撤销它们的 to，不会再有迟到的到达 | 委托、打断 |
| 子 Agent 很快，W0 向 Notify 登记时它已经结束 | 到达在登记的同一个事务里交来，by 是 self，sender 与 via 不变，W0 的取不用改；结束 outcome 的依赖随目标的实际交付传来 | 委托 |
| W0 让监督者知道子 Agent 什么时候结束，但不让它读 Journal | 登记时附带监督者的 Port，它只收到结束状态，拿不到 Look；它的 Port 不收结束状态的，登记当场以 type 失败 | 独立检验、只读观察 |
| 别人冒充子 Agent 的结束交到 W0 的 done | via 不是登记时的那份，sender 也不是子 Agent，按 via 取取不到 | 委托 |
| H 经转交者 F 交给当班的 W，附带回话途径，W 直接回给 H | 来路沿派生链报到 H 自己交出的那一份，H 按回执里的 Handle 取得到；看不到 F 与 W 的 Handle | 交接 |
| 根经一个 Entry 交出两类意见，审批者暂停时第二项到达，中途重启 | 审批者没有 Required port，也不收附带；分别取两个意见，按固定规则给出决议，Notify 交回结果。预期见 Scenarios/Approval.md，独立审批求值器与共用脚手架逐条比对[test:crates/scenarios/tests/approval.rs:approval_uses_an_independent_evaluator_and_recovers_while_paused]，逐提交点恢复相同[test:crates/scenarios/tests/approval.rs:approval_recovers_at_every_commit_with_the_same_decision]；单 Entry 检验意见的处理，多参与者的认证留给系统边界 | 单次调用、固定工作流、打断、只读观察、委托 |
| 根让 W0 改仓库里的一处代码，收到结果写回磁盘；中途客户端断线 | root entry 交付仓库 Bundle，接上模型、文件执行、构造和实例化 Port；W0 显式携带快照，断线重连用原键重报；结果 Bundle 由根在系统外写回磁盘 | 委托 |
| W0 把一处修改交办给子 Agent S，等它结束，审过交回的 Bundle 再采用 | S 的完整 Definition Bundle 与快照由 W0 交付；S 的文件执行从显式输入产生候选 Bundle；W0 向 Notify 登记，按 via 取结束结果，比较基准后明确采用或合并；两份 Definition 共享同一 Library 依赖 | 委托、作用于外界 |
| 根实例化一份两个 Seat 的 Charter：门接 Collective 的 Required port，审计者接门的 Provided port | Collective 与两个成员在一次事件里诞生；成员的 wire 只记 Handle，链与所指从 Charter 的接线与 Collective 的诞生重建；名册不进记录。预期见 Scenarios/Charter.md，只检验诞生，收尾随终止闭环 | Collective |

## 待定

- 是否把契约写成规范，Kernel 是它的一种实现：数据模型、操作、保证、绑定。有了第二个实现或对接方才需要。Definition 与 Language 无关，所以它本身就是通用的 Agent 构造格式；声明式的智能体配置可以作为一种 Language 或前端接进来。没有场景：等第二个实现或对接方。
  - 分层的候选，由下往上：数据（Value、Schema、Bundle、内容寻址）；交互（Port、Reference 作为边、交出与取）；构造（Definition、Interface、Signature），框住认知（Cognition、求值契约）与组合（Charter、Seat、接线），框内不分层；运行时（Instance、Journal、诞生、到达、事件、Control）；授权（获得、附带、撤销权、限定），在运行时之上，因为派生链、被拒、事件都记在 Journal 里。构造在交互之上是推断，未确认：Interface 用 Port，外部服务只有 Port，没有 Definition。
  - 张力：交互是一层，还是横切。跨 block 的形状用 `$block` 引用之后，交互单独成层不必把形状写两遍；剩下的不一致是文件的切法：Reference 同时装着边与授权，构造也向上引用运行时，如 Control 与诞生。
- 系统边界：进来的是 Entry，见「根」；出去的是外部服务，只有 Interface，如外部的 A2A agent。根以外的一方要进入，如别的人、A2A 的服务端，候选是各驱动一个由 root entry 实例化、接线收窄过的 Entry；客户端怎样绑到一个 Entry 上、怎样认证。A2A、ACP、ANP、MCP 这类外部协议，候选是边界上的绑定，由 Plugin 提供；进来的方向上，MCP 把一个 Entry 的能力包成工具，A2A 把一个主体作为对等的 agent 暴露，外部只见到 Entry 一侧的名字，Handle 不出系统（推断）；对照的调研按需启动。委托不是 A2A 的特例：交互是一条轴，结构（产生、来处、Control）是另一条，A2A 只有交互。不同的根就是不同的系统，之间只经边界往来；可信的放弃只在跨系统时才需要，见 Instance 的「同一性」，归这里。没有根在场时谁来开始，也归这里：定时触发、Webhook、外部结果的作者是谁；到达的 `sender` 是 Instance 的身份，系统外的一方没有位置。候选已有，各驱动一个 Entry。场景：定时触发、外部 agent 接入、跨根的承诺，都还没走查。
- root entry 在运行中引入没有来源的 Reference，如新账号：它的 Required port 在诞生时写定。候选：系统外造成的一次授予记在它那里；或换一个新的 root entry，前任运行中拿到的 Control 要它自己交出；或一个 Kernel 提供的 Port，按宿主配置给出新的 Reference，作为结果的附带，只接给 root entry，这样不碰不变量 3，但它是一个新的 Kernel Port，按「放置」要先在运作方式表写出现有组合做不到的一行（推断）。引入新的文件见 Port 的待定「自己算出的内容」。场景：运行中加账号。
- 云上多租户：每个租户一个系统。只靠 Handle 解析不到这一条防线，就把隔离押在 Kernel 没有 bug 上，所以存储、容器、凭据都按系统分开，Kernel 的每个操作都带系统 ID。记录库是每个系统一个数据库或 schema，还是共用并按系统 ID 分：按同一个理由偏向前者（推断）。没有场景：第一版只做本地单租户。
- 还没走查的场景，按检验的运作方式分组；依赖 Plugin 边界的，等边界验证之后再走，接外部服务那一个就是这次验证。常见功能与常见痛点，先调研市面上的做法，再写成场景加进来：
  - 委托、打断：
    - W0 持续对话：根经 root entry 连发几句；W0 正忙；中途改主意，软打断（交来新的）与硬打断（暂停、换引擎）。
    - 委托中途反问：Agent 做到一半要问 Principal。
    - 赛马：同时交办几个异质的 Agent，选最先通过检验的，其余暂停。与已走查的「同时等几个 Agent」的差别在检验：没通过的要接着等下一个。
    - B 直接问 A：W0 交办出 A，把问 A 的途径转给 B，B 带上回话途径；之后 W0 撤销它。
    - 不对称的信任：一方想看就看、想回就回，看没看可以让对方知道或不知道；另一方交来的立即进上下文并调一次模型，或常驻仪表盘，或让对方决定自己怎么看。
    - 运行中构造再实例化：W0 造 D2，D2 的代码依赖取自 W0 自己 Definition 的完整 Bundle，模型碰不到 ID，两者共用同一份不可变依赖。
  - 作用于外界：
    - 并行产生候选：一个 Instance 的几个执行分别从显式 Bundle 输入产生产物，程序决定采用哪一份或怎样合并。
    - 接外部服务：Agent 用一个账号调外部服务，凭据不进容器，也不进 Journal。
  - Collective、独立检验：
    - 群聊房间：成员说话、等轮到自己，房间定怎么说、说多少、什么格式、跟谁说；有人中途进来。
    - 熔断之后：熔断者停了执行者，解除暂停要另一方来做。
    - 审计：检验者读执行者的 Journal 与明确交付的 Bundle，不由被检验者任命。
    - 异质检验：检验者与执行者的引擎必须是不同的模型，由 Charter 保证，不靠选人的一方记得。
    - 四眼：一个动作要两方都同意。
    - 双盲评审：编辑转交，作者与评审互不知道是谁。
    - 陪审团：固定人数、事先指定的候补、计票规则写死。
    - 嵌套：Seat 里放一个 Collective；从外面暂停整个 Collective。
    - 外包的嵌套：供应商又把事分包出去；公司想看到最底下那一层。
    - Principal 参与自己实例化的 Collective：把某个 Seat 的收发 Port 接给自己，或 Charter 的出入口就是某个 Seat，或 debug 时临时切入再归还。
  - 制度形式，Collective、委托：
    - 托管：一方把一项权利交给受托者，按写死的规则使用，结果给第三方，另有一方监督；交出的一方想保留撤销的可能。
    - 特许经营：一方把做法与供给交给另一方经营，想保留检查标准的权利。
    - 介入：项目做砸了，出资的一方要在约定的条件下接手。
    - 角色：一类权利按角色发给几个人，有人换岗时撤销。
  - 交接、程序化：
    - 值班：号码不变，换班的人。
    - 一边生产一边更新：同时有十件业务在做，某类工人要升级，不能原地改造。
    - 改进循环：Principal 用另一个模型提出 Definition 的修改，在相同的输入与初始 Bundle 上试跑候选，比对行为，只采纳有证据的修改，把经验留给下一个任务。
    - 用别人的 Language：一个主体写了一种 Language，另一个想用它写 Definition，不经过根去装。
  - 共享与保密、插入：
    - 限流者、全组预算：几方共用一个服务，按来路分配额。
  - 依赖系统边界：
    - 多人审批：几个人各经自己的 Entry 交出意见，固定的审批 Definition 计票，结果交给申请的一方；全程没有模型。用来检验 Kernel 不依赖 LLM：走查时 Kernel 一处都不用改。
    - 跨根的承诺：出价的一方属于别的根，要确信拍卖的规则中途不会变。
    - 定时触发：每天早上整理一遍收件箱，没有根在场时由谁开始。
    - 外部 agent 接入：外部的 A2A 客户端要交给 W0 一件事，再取回结果。
    - 运行中加账号：根给运行中的 W0 接上一个新的 provider 账号。

参考：Scenarios/Binding.md（同形 Port 的来源）；../README.md（骨架、分类与关系）；Journal.md（待定）；Language.md（待定）；Plugin.md（可信）；Definition/README.md（Cognition 与 Charter）；Instance.md（同一性）；Port.md（待定）。
