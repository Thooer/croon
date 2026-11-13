# Instance

Instance 按诞生时的 Definition 存续，是可变状态的主人。body 携带的状态由已记录结果重放恢复；Entry 的程序位置在系统外。不可变快照可以共享，采用哪份快照是程序自己的状态。需要持续维护正式状态时，由一个主体持有当前位置并按规则提供服务；谁负责沿来处解释。

由 Cognition 驱动的 Individual 一生只求值一次 body。跨多次交互的状态由 body 自己携带，比如 loop 的状态，重放时自然恢复，不需要另设存状态的地方。没有采用的做法是每交来一项就求值一次：那样携带的状态在两次之间就丢了，要另造一处可变状态，还得再规定它怎样声明、谁来接上。Entry 由客户端按求值契约驱动，不求值 body；Collective 没有 body，见「产生与结束」。
## 构成

Instance 是身份加 Journal。身份全局唯一，权利、责任、Journal 都挂在它上面；Journal 用它命名。Instance 不就是它的 Journal：Journal 不行动，不该承担身份与问责。除身份以外，Instance 的状态都由 Journal 决定：

| 状态 | 从 Journal 怎样得到 |
|---|---|
| 来处 | 成员：诞生里写的 Collective。其余：造成诞生的那一步所在的 Instance，那一步是成员做的，按「委托」往上追；root entry 由系统外造成诞生，来处是根，见 README.md 的「根」 |
| Definition | 诞生 |
| Required port 上接的 | 诞生里的接线，加上授予，减去收回与失效 |
| 持有的 Reference | Required port 上接的，加上交出的结果与到达里新得到的，减去失效 |
| 已取得的信息 | 诞生与行动的实际交付，普通内容与 Bundle 根分别索引 |
| 运行状态 | Correct 与结束的记录 |
| 还没取的到达 | 到达，减去被取的 |
| 在等哪些 Atom | 重放 |

这些投影由 Kernel 的索引支持，不是 Journal 的内容。代际号属于实现，对上层隐藏。对一个 Instance 的 Control 不是它的组成部分，而是别人持有的 Reference。

Journal 本身——只追加的记录、记录的形状、四条不变量、恢复就是重放、存储——见 Journal.md。

没有采用给每一层执行单位各设一台状态机再用约束守住：Instance 的状态是 Journal 的投影，一次 llm 或 exec 就是一个 Atom。

## ID

Kernel 分配三种不透明的 ID：Instance 的身份、Handle、事件 ID。它们不是 Value，不按内容寻址。

```json instance.id
{
  "prefix": { "instance": "i", "handle": "h", "event": "e" },
  "separator": "-",
  "alphabet": "0123456789abcdefghijklmnopqrstuvwxyz",
  "max_length": 80
}
```

- 格式：`prefix` 里的种类前缀，接 `separator`，再接 `alphabet` 里的字符，总长不超过 `max_length`。
- 前缀让三种 ID 互不相同，也不会与 Value 的 ID（`sha256:` 开头）、记录里 `by` 的保留值 `self` 与 `outside` 相同。
- 由这个系统的密钥与造成它的位置确定地算出。密钥在系统第一次打开时随机生成，之后不变，是宿主一侧的状态，不进 Journal；此外不用随机数与时钟。所以同一个系统里，同样的输入得到同样的 Journal，重启、重放后 ID 不变。加密钥，是因为位置的熵低：只由位置算，知道算法的一方就能对猜测的位置算出 ID 来比对，事件 ID 就透露了造成它的一方的结构。算法写在代码里：ID 在 Journal 里只是不透明的字符串，以后换算法，已存的 ID 不受影响。


## 程序状态

Kernel 不提供默认工作区或当前产物。程序从 Signature 的输入和依赖开始，执行取得候选结果后决定采用、比较或合并。两个行动可以从同一不可变输入产生不同结果，收取的先后不替程序作选择。需要按先后决策时，用取把先后记成结果。

文件、目录、代码布局由相应提供者与 Language 解释；通用 Kernel 只认识 Value、Bundle、Reference 和行动。一次执行的临时可变环境不跨行动，产物稳定后作为信息交付。

## 执行

- Language 在宿主，按 Definition 写明的那一种、在求值契约下确定地执行，不运行不受信的代码。求值器在系统外的 Instance，如 root entry，Kernel 不运行它，也不靠重放恢复它，见 Definition 的求值契约。
- 介入求值只在 Atom 之间生效；Kernel 结束 Instance 时按下面的终止规则收尾在途的 Atom。一次调用中途打断会损害推理，所以正在进行的模型生成只能中止、丢弃，不往里注入东西。
- 每次取得执行权，代际号加一，旧会话不能提交新行动。同一 Instance 只允许一个求值代际；已接受执行保留接受快照，收齐或安全回收后才能换代际。
- 求值异常时，停下还是结束，看能否在 Definition 不变、语义承诺不变的条件下修复原因。登记的 Language 可以修正实现以符合原来的语义；按内容寻址钉住的求值器，改代码就要换 Definition，两者据此区分。
  - 宿主故障是崩溃，不是失败：进程退出、重放分歧、宿主一侧给求值器的兜底（如时间、内存）耗尽，都不写正文失败。登记 Language 的单元故障会丢掉会话并保留请求与结果，宿主标记停下；其他 Instance 仍可行动，显式重开后从 Journal 接着走。同一处反复故障报告并停下；修好实现后同一个 Instance 能接着走。
  - Definition 写出的资源用尽是失败，见 Definition 的求值契约。需要换 Definition 才能修复的求值故障，用 Kernel 造成的结束；不受信 Language 的声明与判定仍在 Language.md 的待定。
- 登记 Language 的宿主故障只停目标的求值：丢弃 Session 与隔离单元，保留代际、在途请求、未送回的结果与已提交事实。轮询先检查宿主停止标记，不因没有 Session 自动重开；其他 Instance 与 Correct 照常行动，Look 仍为 running 或 paused，Notify 不伪造结束。
- 接受快照固定原请求、解析的来源和附带、派生链、输入范围、通道、代际及提供者恢复身份。进程内经原 Execution 收取；重启按固定提供者身份恢复，不能用当前来源上的新绑定替代。并行执行各用自己的输入，不争抢当前工作区。
- 收取故障局部停下，保存请求、接受项和 Execution。暂停及宿主停止期间继续收齐已接受的有限效果；无世界效果的执行可丢弃，解除暂停后重新计算。收齐后内部求值由宿主显式重开，Entry 同键重报继续。
- 外部重放分歧只关闭客户端会话，已接受执行仍按固定代际提交。结果完成但记录库暂不可写时保留完成对象，只重试提交，不重新收取、启动或产生新的产物。
- Kernel 结束一个 Instance，先封住新行动并停止在途的执行，再把全部结果与结束在一个事务里写下。每个已接受、还没有结果的 Atom 都在自己的 Journal 里收尾，保留原来的键、目标、值与附带：交出以 terminated 失败，稳定产物放在统一结果的依赖里；经出站代理已放出却无法确认结果的语义仍在联网待定；取的结果为空。取的空结果由紧随其后的结束说明原因，不表示它的 timeout 到期。随后写结束，outcome 是 terminated，Language 不再恢复，所以接不住这些收尾结果。之后按 Notify 上的登记交出结束状态，Provided port 失效。它没有可调用的强制终止 Port；持有 Correct 的一方仍用暂停加收回来纠正。
- 终止记录的 by 指向造成终止的一步，不从 Atom 的执行者推定。Collective 结束时，原因是占着 Signature 所在 Seat 的成员的结束：被截断的 Atom、成员的结束、后续到达与失效都带那次事件的 ID。即使 Atom 记在另一个成员的 Journal 里，by 仍是那个造成终止的成员。Kernel 不作为新的行动者出现在 by 里。
- 有效果的 start 之前持久化接受项与物理 attempt ID。提供者 start 同一 attempt 幂等，retire 必须封住迟到启动、停止全部写者；retire 确认后该 attempt 永不复活。逻辑行动是 (Instance, key)，安全回收后重做用新 attempt。缺少原恢复提供者或不能确认回收就停下。
- 恢复先查已提交结果：有结果只做幂等清理；没有结果先 retire 旧 attempt，再按固定接受快照从输入重做，不重新授权或解析接线。产物先落盘再提交结果和依赖。宿主占位随结果事务标为已提交，清理成功后删除。
- Kernel 造成结束时，在任何物理动作之前持久化终止意图、原因事件、受影响的 Instance 与在途行动。恢复先续同次终止，禁止重开这些 body；全部收尾和结束在一个事务提交。嵌套 Collective 共享原原因事件，不能中途产生第二个行动者。
- 进程内存里长驻的外部服务状态仍属于外部世界；不借本地快照冒充可以恢复其真实效果。


## 产生与结束

实例化产生一个 Instance：Kernel 写下它的诞生，记着完整 Definition Bundle、输入交付和每个 Required port 接上的 Reference。它立即开始运行，实例化的一方不等它。实例化的一方得到它每个 Provided port 的 Reference，加上对它的 Look、Notify 与 Correct，可以分开转交。来处有两种：

- **委托**：实例化它的一方是 Principal，它是 Agent。实例化的一方是成员的，Principal 是它所属的 Collective，见「委托」。Agent 是这个位置，不是主体的同义词；成员的来处是 Collective，不是 Agent。
- **成员**：随所属的 Collective 一起产生、一起结束。

沿来处往上只有一条路，到根为止；路上经过一个 Instance 的，都是它的后代。

由 Cognition 驱动的 Individual 在 body 求值结束时结束：结果按 Signature 的 out 检查，Kernel 写下结束，在同一个事务里按 Notify 上的登记交出结束状态。之后它的 Provided port 失效：持有指向它们的 Reference、自己还没结束的一方各记一条失效，再交给它的被拒；已结束的一方的 Journal 到结束为止，不再记。对它的 Look 与 Notify 仍然有效：查看、读取照常，向 Notify 登记当场交出结束状态；对它的 Correct 以 denied 失败，已经没有可纠正的。它的 Journal 保留，可以读取，可以追责。

Collective 没有 body，也没有自己的一步。实例化一份 Charter，Collective 与它的全部成员的诞生是一次事件，在同一个事务里写下：Collective 的诞生写着人选，即每个 Seat 由哪份 Definition 来占；每个成员的诞生写着所属的 Collective、占的 Seat、名册，以及按 Charter 接上的 Reference。占着 Signature 所在 Seat 的成员结束，Collective 就结束：结果按 Collective 的 Signature 的 out 检查，结束状态按 Notify 上的登记交出；其余成员由 Kernel 当场结束，见「执行」，这也是同一次事件。别的成员先结束，只是它的 Provided port 失效。Collective 的 Journal 只记它自己发生的事：诞生、结束、施加在它身上的 Correct；成员的诞生与结束记在成员自己那里。一个 Collective 发生过什么，是它与它的成员的 Journal 的投影。


## Control

Control 是 Kernel 提供的、作用在一个 Instance 上的三个 Port：交给它们的由 Kernel 执行，目标的 Definition 不经手，拒绝不了。

| Port | 操作 | 作用 | 在目标那里留下记录 |
|---|---|---|---|
| Look | 查看 | 不等待，得到它的运行状态与结束状态 | 否 |
| Look | 读取 | 读它的 Journal。读到的只是信息：其中的 Handle 不在读的一方的 c-list 里，用不了；内容与依赖只按所选记录的实际交付集合传给读的一方，见 Reference 的「能读的 ID」 | 否 |
| Notify | 登记 | 它结束时，Kernel 沿登记者附带的 Port 交出结束状态；已经结束的，登记时就交出 | 否 |
| Correct | 暂停 / 解除暂停 | 停在 Atom 之间 / 继续 | 是 |
| Correct | 授予 / 收回 | 把一个 Reference 放进它的某个 Required port / 拿掉 | 是 |

- Control 交给实例化的一方；持有者可以把它们分开转交，如 Look 给审计者、Notify 给等结果的一方、Correct 给熔断者。执行者不持有指向自己的 Correct，修正权由此独立。成员实例化的 Instance，来处是 Collective，Control 仍在实例化它的成员手里：Collective 没有 body，拿到也用不了。
- Control 不沿实例化往下够：持有对 A 的 Control，够不到 A 实例化出的 B。要够到，由 Principal 组合，如让一个保管者代为实例化并保管 Control；够到多深、在什么条件下够到，是决策权的分配，因场景而异，没有普遍最优。A 要是能从别处拿到 instantiate，就绕得开；它的 Port 不接受任意附带时，这个上限构造时算得出。不把可达定为默认，还因为以后给已发出的 Control 加上可达，是事后扩大权利；以后要原生的可达，是一种显式申请的新 Control，是增量。全局调试同理：根要在主体这一层看全部 Instance，就这样组合；否则是宿主上的事。
- Control 分成三个 Port，不靠收窄一个 Port 来分：每个 Port 的附带声明是固定的；等结束最常见，只要结束状态，读取给出整本 Journal，分开就能只给前者；「执行者不持有指向自己的 Correct」只看 Reference 指向哪个 Port，是结构上的事实，不必分析一份收窄过的 Port spec 还能不能纠正。Look 与 Notify 在目标那里不留记录，Correct 留。一个 Port 里再收窄，如只能暂停，是一份更窄的 Port spec。
- 对 Collective 的 Control，Correct 只作用在整体，Correct 的 in 里没有路径：暂停、解除暂停是对全部成员的一次事件；授予、收回 Collective 自己的一个 Required port，改接所有接到它的位置。Look 与 Notify 可以带一个 Seat 的路径，指到占着它的成员；那个成员是 Collective 的，可以再往下指；不带路径的对的是 Collective 自己。理由：带路径的授予、收回、暂停，能在 Charter 的 ID 不变时把结构改掉，如撤掉审计者的 Look、把分开的 Reference 凑到一个成员身上，依赖这份 Charter 的一方就被骗了。要单独纠正某个成员，写进 Charter：把它的 Correct 接给熔断者或管理 Seat。成员之间的接线怎样才会变，见 Definition 的 Charter 刚性。
- 施加 Control 是施加者的一次交出，记在施加者的 Journal 里；暂停、解除暂停、授予、收回在目标那里是别人造成的记录。
- Control 在 Atom 之间生效。暂停时已在途的 Atom 看有没有世界效果：没有的（模型、取）丢弃，位置回到它之前，解除暂停后重新执行；有的（exec）等它结束，最长到它的上界，结果记录排在暂停记录之后，见 Reference 的「通道」。暂停后默认不解除，必须显式解除暂停，熔断就是这样做的。换它接上的东西，就是暂停、收回、授予、解除暂停。
- 打断有两种：交到它的 Provided port 上，由它在两步之间取到；或者暂停它。两者都不需要新机制。
- 等一个 Instance 结束：交给它的 Notify，附带一个指向自己某个 Provided port 的 Reference，再从那里取。登记与送达都是普通的交出：目标结束时，Kernel 沿附带派生出的那一份交出结束状态。交来的到达：via 是那一份，登记结果的 derived 给出它的 Handle，所以按 via 取就只取到这一个的结束；value 是结束状态，形状见 definition.outcome；sender 是目标；by 是造成结束的那一步所在的 Instance，目标自己结束时就是目标，因 Collective 结束而随之结束时沿同一次事件。登记时目标已经结束、当场交来的，造成它的是登记那一步，by 是 self。那一份只用一次，交过就丢掉；退订就是用登记结果里的撤销权撤掉它；送达前已经失效的，不送达。附带的也可以是别人的 Port：对方只收到结束状态，拿不到 Look。不另设等待：一次取就能同时等对话与几个 Agent 的结束，只等一个的等待写不出这个。
- 读自己的 Journal，要实例化的一方用 self 把自己的某个 Required port 接到自己的 Look 上，与指向自己的 Reference 一样，不天生就有。
- 不设解除不了的强制停止：暂停加收回已足以强制纠正，因为在途的 Atom 要么能随时丢弃，要么有上界。


## 委托

- 要不要委托、委托给谁、Agent 是什么样，由主体自己选：从已有的 Definition 里选，自己构造一份，或请别人造。不满意就换。它能做到哪一步，看它接上了哪些 Port。别人通过对话要求它委托，只是提供信息。
- Agent 对交付的结果负责，也对自己发起的委托负责。它委托出去的 Agent 属于它，不属于它的 Principal。
- 结果以数据与显式依赖交回；文件快照也是 Bundle，由接收方决定采用或合并。Agent 的独立执行不修改 Principal 携带的状态。
- "拥有一个主体"分三样：持有它的 Provided port（能把活交给它）、持有它的 Control（能纠正）、是它的来处（对它负责）。前两样是 Reference，可以交给不同的主体。要让使用者碰不到某个 Reference，Agent 的来处应是持有它的一方，再把 Provided port 转交给使用者。Charter 里的上级至多有前两样，而且是 Charter 写明、收窄过的；第三样归 Collective。上级扩大不了自己的权利，要改就换 Charter；下级听不听，是下级的 Definition 怎样对待交来的东西。
- 成员没有自己的财产：它的权利全部来自 Collective，经 Charter 接给它，所以它用这些权利产生的 Instance，来处是 Collective，Principal 也是 Collective；Journal 里记着是哪个成员做的。就像员工盖公章签的合同属于公司。所属的 Collective 本身也是成员的，再往上，直到经委托产生的那一层。


## 同一性

同一性挂在 Instance 上：Instance 不换 Definition。改变工作方式，就是由一个按新 Definition 行动的新 Instance 接替，前任停止行动。前任的 Journal 保留，可以读取，可以追责。

没有采用的做法有两种：

- 把同一性挂在工作上：权利、责任、历史都在 Instance 上，追责要沿 Instance 一直追到底。
- 让一个主体跨越不同的 Definition 延续：Journal 要按 Definition 分段，Interface 随时间变化，换 Implementation 时别人手里的 Reference 要失效。别人依赖的连续，其实是一个地址：由一个主体提供，别人持有它的 Port，它把交来的转给现在做这件事的 Instance，这个地址上的历史就是它的 Journal。以后转交的那一跳成了负担，Kernel 可以按那个主体给出的路由直接解析，语义不变，是增量。委托之下没有 Seat，Agent 属于它的 Principal，换 Agent 就是换一个人；Charter 的成员也不换，见 Definition 的 Charter。

不存在原地修改正在运行的 Instance。Definition 是运行时的数据，构造完就能实例化，不需要编译，所以效果上可以随时改。Language 写在 Definition 里，所以换 Language 也是接替；旧 Instance 按旧 Language 重放，不受影响。

运行中的改变有三种：

| 谁改 | 改什么 | 靠什么 |
|---|---|---|
| Agent 自己，在给定的范围内 | 用哪些工具、看哪些说明 | Definition 里的结构 |
| 持有 Correct 的一方 | Agent 接上的东西，如换引擎、加一层批准 | Correct：收回与授予 |
| Principal | Agent 的 Implementation | Handoff：换一个新的 Instance |

换接上的东西，改的是 Competence，不是 Implementation，所以不换 Instance。面向 Principal 的开关（如调试）就是 Control，不进 Interface。

量变与质变由此分开：换 Reference 是量变，在同一个 Instance 里；换 Definition 是质变，是新的 Instance。量变攒不成质变：Correct 只换持有 Correct 的一方给出的东西，见「Control」。没有系统能功能无限扩展又线性演化，版本迟早要不兼容地重构，所以 Instance 之间的界线要硬：前一个结束，指向它的 Reference 失效。

别人能依赖的同一性有两种：Instance，即持有它的 Port，结构不变，结束就失效；地址，即持有转交者的 Port，一直在，背后的 Instance 会换。要承诺就持有 Instance，要连续就持有地址。

一个根之内不设可信的放弃：给出的一方总留着撤销权，实例化的一方总留着 Control。依赖一个结构的一方要的是两条：看到的结构就是运行的结构，由内容寻址与接线不变给出；结构变了看得见，因为质变只能换 Instance，结束是一次事件。这两条都不需要放弃。反过来，结束与替换一个 Instance 的权力必须有主人，放弃了，换 Instance 就没人负责。Principal 想防将来的自己被说动，就把规则写进 Charter，或让上游持有检验者；事后追究靠 Journal。可信的放弃只在没有共同的根时才需要，是系统边界上的事。

Handoff 时，前任的东西按来处分别到继任者那里：

| 前任的东西 | 例子 | 怎样到继任者那里 |
|---|---|---|
| Principal 经 Interface 给的 | Required port 的接线、Signature 输入与依赖 | Principal 直接再给一次：这些它本来就持有 |
| 前任运行中得到的权利 | 附带、交出的结果带来的 Reference、它委托出去的 Agent 的 Control、还没取的到达 | 属于前任，只能由前任交出，即收尾 |
| 前任留下的信息 | 已交付产物、Journal | Principal 经 Look 取得，再以输入数据与依赖交给继任者 |
| body 携带的状态 | | 不传：前任的原始轨迹不给继任者 |
| 别人手里指向前任的 Reference | | 前任结束就失效；要连续，事先让别人持有一个转交者的 Port |

前任不配合，比如已被说动，Principal 就暂停它；它运行中得到的权利留在它那里。

**改 Agent 的工作方式由它的 Principal 来做**：修改 Definition、测试，再 Handoff。理由有两条：

- 引擎没有自己的检验。自我改进会发明不存在的失败并为它加规则，只看自评发现不了。
- 信息不对称。Agent 只看得到 Definition 给它看的投影（上下文、提示、工具描述），看不到 Definition 本身；Principal 选定了它、持有它，也有手段测试它。

Handoff 要确保前任停止行动；不配合收尾的，由 Principal 经 Correct 暂停。Agent 不持有指向自己的 Correct，不能自行完成这一交接。Principal 给 Agent 接上"读自己的 Definition"和"构造"，Agent 才能自己提出新的 Definition，采不采用仍由 Principal 决定。

## 待定

- 持久终止与嵌套 Collective 的实现证据随 Charter 场景补齐。设计按「执行」的同次终止意图落实，不能在写者仍存活时承诺结束。
- 实例真正结束后，它持有的 Reference 怎么办：它的 Agent 随之结束，还是交给来处。正文只定了别人手里指向它的 Reference 失效。它委托出去的 Agent 的 Control 会没人能用，成员被暂停后同样，除非 Principal 组合了 Control 的保管者。来处结束后留下的 Instance 可能还留着上下文、还能与别人交流；没有需要，不收紧。没有场景。
- 成员对外行动时，外面看到、记下的是谁。现在两处不一致：它产生的 Instance，来处是 Collective，「成员没有自己的财产」在 Control 交给实例化的一方之后只剩问责的投影；它交出的东西，到达的 sender 是成员自己。Seat 里是一个 Collective 时，交来的是它的某个成员，名册里写的却是 Collective，对不上。候选：一律记 Collective，成员对外不可见；或一律记成员，来处是实例化的一方，问责沿成员关系往上。身份只是信息，不带权利。场景：公司的采购成员外包给供应商、外包的嵌套、嵌套、双盲评审。

参考：Language.md（待定）；Value.md（Bundle）；Definition/README.md（Signature、Port、附带、Charter、Seat）；Definition/Cognition.md（求值契约、诞生、Atom、失败）；Journal.md（记录、四条不变量、恢复、实现）。
