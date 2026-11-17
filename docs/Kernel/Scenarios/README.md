# 场景

这里写一组场景检验什么：Kernel 跑完之后，每本 Journal 应当是什么样。场景本身（谁想做什么）在 ../README.md 的场景表，本目录不放设计。每个场景一个文件；预期是机器读得懂的 block，名字是 `scenario.` 加文件名的小写，在 crates/scenarios/schema/ 下。用真的 Language 写的场景，预期放在用它的上层仓库里，名字前面加那个仓库自己的前缀（croon-app 的是 `library.scenario.delegate`），与这里的场景共用 Value 的名字空间，格式与脚手架相同。

## 格式

一个场景是一个对象：

- `journals`：Instance 的占位名 → 预期的 Journal，记录按 instance.record，不写 `seq` 与 `at`：seq 是位置，与 instance.record 一样从 0 起，at 由脚手架补。元素是数组的，就地展平，所以几个场景共用的前缀写成 `parts` 里的一个数组，别处 `$block` 引用。
- `values`：Value 的占位名 → 内容。所有场景共用一个名字空间：同一个名字在哪里都指同一份内容，只在第一次需要时写一处。
- `parts`：只供 `$block` 引用的片段，脚手架不读。
- `pending`：可选，Instance → 它在 Journal 之后报出、到最后也没有记录的 Atom，形状是 definition.eval 的 atom，如暂停时丢弃的生成。
- `order`：可选，`[{ "first": [Instance, 序号], "then": [Instance, 序号] }]`，跨 Journal 的先后由事件推不出时才写。
- `perturb`：可选，扰动，见下文；扰动的预期是 Journal 不变。

占位名：Instance、Handle、事件照 instance.id 的格式写，如 `i-w0`、`h-w0done`、`e-w0born`，只在一个场景里有意义；Value 写 `sha256:` 加一个不是六十四位十六进制的名字。比对时占位名与真实的 ID 一一对应地绑定：同一个名字只绑一个 ID，不同的名字不绑同一个，所以内容相同的 Value 用同一个名字。

只写推不出的：

- Bundle、交付依赖与产物都写在 `values` 里；ID 由规范内容算出，依赖数组排序且无重复。普通内容里的 ID 不自动成为依赖，脚手架不为缺失内容造占位对象。
- Port spec 与形状用 `$block` 引用 reference.kernel、scenario.engine、scenario.exec 等已有 block，不抄。
- 源码与目录不抄进 JSON：Value 的内容里出现的 `{"$file": 路径}`、`{"$bytes": 文本}` 和 `{"$tree": 路径}` 由脚手架展开为内容与 Bundle；路径相对仓库根。
- Definition 的 language 是 `fake@1`：只供脚手架登记的假 Language，body 是一个说明用的字符串，不求值。
- 失败的 message 与 data 还没有规定，写 `""` 与 `null`，只比 kind。

## 脚手架

一个简单的脚手架拿一个场景驱动真的 Kernel，在共用的测试 crate crates/scenarios/ 里，Kernel 的场景与 Language 的场景都用它：[test:crates/scenarios/tests/scaffold.rs:scenarios_run_on_the_real_kernel]

- **root entry**：脚手架是它的客户端。按它的诞生创建它：完整 Definition Bundle、输入交付和 binding 取自 `values`；self 里的 Required port 接到自己。之后按它的 Journal 报出 Atom。
- **假的 Language**：一个 Instance 诞生时，按造成它诞生的事件找到它在 `journals` 里的那本，依次报出其中自己的步：交出给出 key、to、delivery 与附带的目标，取给出 key 与 filter；步走完，报出 `pending`，没有 pending 就以 end 记录里的结果报 done。
- **真的 Language**：Definition 的 language 不是 fake@1 的、求值器在宿主的 Instance。Atom 由它报出，脚手架管不着；先后改由假的 llm 与 exec 何时给出结果来定。交给宿主源的按绑定目标识别；场景中的模型与 exec 由上述测试 provider spec 解释。一次结果只在它的记录能走、并且同一本 Journal 里紧接着的、由 Kernel 当场完成的那几条（交给 Kernel 自己执行的 Port 或 Instance 的交出、结束）也都能走时才给出，到下一个取、下一个交给 llm、exec 的交出或别人造成的记录为止；能不能走按下面的「先后」判。当场完成的那几条，先后由 Language 定，脚手架控制不了：只靠控制报出才得到的先后，如 Quick 里子 Agent 先结束，用真的 Language 复现不出。[test:crates/scenarios/tests/scaffold.rs:a_real_language_reports_its_own_atoms_in_any_compatible_order]
- **假的只接在三处**：Language 的插口，fake@1 经 definition.eval 往来；模型的提供者；exec 的容器。其余都是真的 Kernel：内容库、记录库、索引、决定与提交，见 ../Journal.md 的「实现」。root entry 的客户端在进程内直接调 Kernel，不经 socket。
- **假的宿主 Port**：按来源 Handle 装配，接受一次给一个独立执行对象，按交出记录里的 result 收取；隔离执行的结果显式交付产物 Bundle。pending 里的 Atom 不回。
- **先后**：一本 Journal 里按序号；跨 Journal 的，事件造成的记录排在造成它的那一步之后，再加上 `order`。脚手架按任何一个与这个偏序相容的全序推进，得到的 Journal 都应当相同。[test:crates/scenarios/tests/scaffold.rs:any_compatible_order_gives_the_same_journals]

  测试不可能枚举全序，所以只跑 seed 1 到 8，每场景各一次；名字里的「any」指每次得到的结果都得一样，不是指覆盖了全部。16 次 Index 重建同理，只随机交错不同 Journal 的记录，不重排一本 Journal 内部。这两处都靠上面的偏序证明覆盖，而不是靠枚举。
- **预存**：需要排除「ID 不存在」干扰时，Setup.preload 在 root entry 创建前导入指定内容与 Bundle，只存内容，不授予 Instance 取得资格。

测试用的假提供者 Port 的形状：模型是 `scenario.engine`，文件执行是 `scenario.exec`。

- **比对**：每本 Journal 在占位名的绑定下与预期逐字段相等。

脚手架之前，crates/scenarios/tests/expected.rs 先查预期本身合不合文档里的规则：记录、Definition、四条不变量、取与到达的对应，以及交出的值与结果合所经的 Port spec；用真的 Language 写的 Definition，展开之后还要过它的构造检查。

两条容易漏的装配：

- **通道**：假宿主 Port 的通道由 Setup.channels 按 host 名给出，不从 Port spec 推。spec 相同而通道不同的两个 provider 得各写一行；名字不在表里的 host 不会被绑定，也不报错，新增一个 host 就要同时登记它的通道。
- **provider role**：真的 Language 的一次交出算不算交给提供者，由 Setup.provider_roles 按 host 名给出，默认不是。据它挑在途崩溃点的测试要先把负责该 Port 的 host 登记成 Provider，否则这次交出不被计入，崩溃点少测一处而测试仍然通过。

核心场景可独立运行：`cargo test --offline -p scenarios`，不依赖 Lang0、Library 或 App。各场景自己的 schema 由 scenarios 直接编译取用，Kernel 构建不嵌入场景。Lang0 和 App 的集成场景由对应仓库负责。

## 扰动

- `{"crash": {"instance": I, "after": n}}`：I 序号为 n 的记录提交之后 Kernel 进程退出，重启后恢复。`{"crash": {"instance": I, "key": k}}`：k 这个 Atom 在途、结果提交之前退出，恢复时按通道重做。脚手架本来就要对每个提交点各崩一次，所以只写要指定的位置。
- `{"reconnect": {"after": n, "resend": [键], "stale": [键]}}`：root entry 序号为 n 的记录提交之后客户端断线，新会话重报 resend 里的键，得到同样的结果，不重做；旧会话之后才报的 stale 里的键被拒，不记。

参考：Binding.md（同形 Port 的来源）；../README.md（场景）；../Journal.md（记录、四条不变量、实现）；../Instance.md（ID）；../Definition/Session.md（会话）。
