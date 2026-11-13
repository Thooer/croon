# 实现

实现上的选择与理由。设计在其余各部分；这里只记跨部分的用什么做、为什么，以及为以后留的接口。绑某一部分的选择就近写进那一部分的「实现」一节：存储见 Journal，内容库见 Value，容器、模型与凭据见 Reference，Language 的接口见 Language.md。内部实现只写在代码里。

## 选择

选择的标准是核心功能要不要语言的特性、核心要用的库哪个语言做得好。核心指保证依赖的那部分代码：存储、数据模型、通道、Language 插口。不在核心里的，如接 provider，走 HTTP 或 FFI，不影响选择。

繁琐、封闭的工作交给稳定、高质量、不限制灵活度、没有隐藏副作用的基础设施，不自己造轮子。依赖一律关掉默认特性，只开需要的；不隐式读环境变量，不联网；版本钉死，Cargo.lock 进 Git；检查会改变全局语义的特性有没有被打开。只依赖文档承诺的公开接口；文档没承诺的行为（默认值、计数方式、回收时机），能显式设定就设定，能自己做就自己做，都不行才依赖，并写明版本，升级时重查。

| 部分 | 选择 | 理由 |
|---|---|---|
| Kernel 的宿主语言 | Rust | Kernel 的数据几乎全是带标记的联合（记录、Atom、结果、目标、来源、Control 的操作），以后还会加种类；enum 加穷尽匹配，加一种时编译器指出所有要改的地方。Lang0 的求值器经 definition.eval 往来，宿主语言另选，不影响这一选择 |
| 基础库 | tokio、serde_json、sha2、thiserror、tracing 与 tracing-subscriber、rusqlite；版本见 Cargo.lock | 各管一样封闭的工作：异步运行时、JSON、内容寻址的哈希、错误类型、日志、本地记录库（SQLite 的源码编进来，版本由 Cargo.lock 钉死，不依赖系统的库） |
| 发布版本 | SemVer，现在是 0.x.y | 当前直接修订契约、数据和测试，不保留开发期旧格式解释。兼容以功能可组合检验；运行中的不可变身份、记录和重放仍严格成立 |
| 校验 | 自己写，只认 value.schema 的子集 | 报错要带路径、说出违反了哪条规则，并用模型写的写法说话（如 Lang0 的 TypedDict、Literal）；通用库的报错还要再翻译一层，工作量不小于一个十个关键字的校验器。先用 value.schema 校验每一份 Schema，未列出的关键字就被拒；再校验值 |
| 生成器 | 先只做取出：每个带名字的 block 平铺成名字到文本的表，不区分 Schema、注册表与数据，怎么用由 Rust 代码决定；取出时展开 `$block`，表里是展开后的文本，规则见仓库根 AGENTS 的「取用」。类型的生成等 Kernel 需要时再写，自己写，只认文档里用到的几种 anyOf：按键区分的联合、带字面量判别字段的联合、按 JSON 类型区分的联合（如 `boolean \| Schema`）、`T \| null`；别的写法报错，指回文件与 block 名字 | 取出不解读，所以 block 不用标记自己是哪一种，形状仍只在文档里一份。展开在取出时做，Kernel 的校验器与交给 LLM 的 Schema 看到的都是自身完整的一份，value.schema 不加关键字。typify 0.8 损失两处：单值的 enum 变成 bool，definition.outcome 的 ok 分不开 true 与 false；跨 block 的形状变成任意 JSON。只认几种写法，规则就是结构 |

## 进程形态

- Kernel 是常驻的守护进程：Instance 实例化后立即运行，可能无限期地等着取，根也要随时能交东西进来。命令行、TUI、Web 都是客户端。
- 本地 API 走 Unix socket，没有认证，只靠 socket 的文件权限挡住本机的其他用户，所以不开 TCP 端口：开了，本机的其他进程与浏览器里的网页都能连上来。云上同一套 API 加 TLS 与认证。
- API 就是 root entry 的会话，加上按 ID 读 Value，语义见「求值器在系统外」：实例化 W0、往它的 Port 交东西、收到回复、读 Journal，都是 root entry 报出的 Atom。此外只有几件宿主上的事，见「根」：凭据，装 Language 与 Plugin，创建 root entry，Kernel 的只读界面。

## 接口

宿主按来源 Handle 接上 Port，按提供者恢复身份保留接收已接受执行的实现。通用接口不暴露目录、当前工作区或整个 ContentStore。以下边界必须由实现兑现：

- Read 提供 json、bytes、bundle，业务读取带已消费的结果键；构造读取固定为 Definition 闭包。读取结果以 HostError 区分宿主故障与不可读，不能把损坏或预算耗尽悄悄变成内容不存在。
- Content 提供本次输入范围内的读取，以及 put_json、put_bytes、pack。新写内容和 Bundle 在本次执行范围中可读，结果提交后才成为调用方的资格；对象使用 Rc 持有，便于异步执行保留本次范围。
- Call 固定逻辑位置、来源、代际、物理 attempt 和输入交付。Port 固定恢复身份与 Channel；start 返回 Execution，collect 返回完成结果或待续，retire 封住迟到启动并停止写者。Discardable 不需要持久 attempt，Isolated 要先写宿主接受项。
- HostDone 使用 definition.outcome，依赖根先按 Content 的来源验证。宿主 Port 本轮不返回 Reference，Kernel 的结果附带仍使用共同契约。
- 配置显式传入每次节点、读写字节预算；Execution 只持有本次 Content，没有任意内容库读取方法。

宿主接受项固定请求、解析引用与链、原提供者身份、输入根、通道和代际；每次物理启动有新 attempt，重试收取不新建 attempt。先 retire 原 attempt，确认不会迟到启动，才允许从原输入重做。提交与占位标记在一个记录库事务完成，之后幂等清理。

终止意图先落盘，再停止实际执行；它固定原因事件、目标集合与在途请求。恢复优先完成终止，再考虑重开求值。宿主占位不进入业务索引；既有行动事实仍只在 Journal。

接口的 JSON 形状在各部分取用；Rust trait 的内部组织只在代码。上面列的是跨模块保证，不规定方法参数的第二份形状。

规格也按消费方取用：Kernel 构建只读核心规格，场景的集成装配按需加入具体 Language 与 Library。各 crate 直接从自己的 `schema/` 文件读取形状，移动文档不改变代码使用的规格。

## 待定

- 其余库写到时定，以后能换，候选（待核验）：本地 API 用 axum；模型的 HTTP 用 reqwest；云上 PostgreSQL 的驱动，与本地的 rusqlite 共用一份 SQL 文本，或整体换成能接两种数据库的 sqlx；容器用 bollard；凭据用 keyring。场景：Reconnect 的真实版本，即真的 socket、模型与容器，在首刀之后。

参考：Port.md（Kernel 的 Port）；README.md（根、保证）；Journal.md（实现）；Reference.md（通道、能读的 ID）；Language.md（插口）；Definition/Cognition.md（求值契约）；Instance.md（执行）。
