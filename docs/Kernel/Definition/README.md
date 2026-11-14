# Definition

Definition 是 Bundle 中的 JSON，规定主体的 Interface；有 Implementation 的，还固定内部做法。专指一份 Definition 的 ID 时，写的是完整 Bundle ID。

- **不修改，只替换。** 改变工作方式，就是构造一份新的 Definition。这样，已经发生的事有确定的来由；授权只收窄，不需要为自我修改开例外；制造别的主体和改变自己是同一种操作。
- **不行动，所以不带权利。** 构造一份 Definition 不接触世界，不需要授权检查。但运行中的主体要构造，仍得接上构造的 Port：能不能造东西，和其他运行时的灵活性一样，由接线决定。同一份 Definition 可以约束多个主体。
- **是数据。** 冻结、比较、复用、运行前检查，都要求它是能查看的数据。它比任意程序约束更强，比配置表达力更强。Croon 只接受 Definition 的数据；宿主代码里的构造器只是前端，由可信的库作者在 Croon 之外运行，产出的仍是数据。
- **承载功能，不承载意志。** 它只被实例化：功能要一个 Instance 来兑现，一个 Principal 来拿到，见 ../README.md 的「主体」。一份 Definition 不嵌进另一份里求值。

组成形式与名称关系见 ../../README.md 的「分类与关系」。本文讲 Interface、Implementation 与 Charter；Shape.md 给出形状与构造检查，Cognition.md 给出求值契约、body 的边界与失败。

## Interface 与 Implementation

- **Interface** 是外部模型：别人能怎样与它交互，别人依赖的就是它。
- **Implementation** 是随 Definition 钉住的内部做法：Cognition 规定 body 的求值，Charter 规定成员结构。Entry 的做法在系统外，Definition 只有 Interface 与外部求值标记，规则见 Cognition.md 的「求值器在系统外」。

Interface 由描述、Signature、Provided port、Required port 组成，各对应一种依赖它的关系：

| 组成 | 谁依赖它 | 是什么 |
|---|---|---|
| Signature | 实例化的一方 | 开始时给什么，结束时得到什么 |
| Provided port | 持有指向它的 Reference 的一方 | 能交给它什么；零个或多个 |
| Required port | 接线的一方 | 它工作需要接上什么，按本地名字声明，如 `engine`、`exec` |

Signature 是一份 Definition 对开始与结束的约定：in 是诞生时的输入，out 是这一生的结果，都由 Kernel 检查，结果由持有 Look 或 Notify 的一方得知。Signature 不是 Port：没有指向它的 Reference，in 只在开始时给一次，out 只在结束时给出一次。

Signature 约束交付的 value；开始与结束都可以明确交付 Bundle 依赖，不带 Reference 附带。开始时要给的 Reference 接在 Required port 上；结束时要交回的 Reference 沿一个 Port 附带出去，如 Principal 实例化时接上、或交办时附带的回话途径。要交回的服务还没有状态的，只交回它的 Definition 的 ID，由 Principal 自己实例化，来处与 Control 都在 Principal。理由：结果由持有 Look 或 Notify 的一方得知，out 带附带，向 Notify 登记的一方就拿到了权利；只交给实例化的一方，又成了按身份给权利，Control 也可能已经转交。

Port 有一份描述、in、附带的声明，有的还有 out。Port spec 单独成一个 Value，附带随身带上它。Port spec 可以递归，如回复里又带回复：顶层的 `$defs` 放命名的 Port spec，叶子写 `$ref` 指过去，顶层本身也可以是一个 `$ref`，与 value.schema 的写法一样；in 是一份 Value 的 Schema，有自己的 `$defs`，两层各管各的。两份 Port spec 相同，指去掉各处的 description 之后规范文本相同；含 `$ref` 的，`$ref` 按它所在那份 Port spec 顶层的 `$defs` 解析，所以连同顶层一起比：两份 Port spec 的顶层去掉 description 之后相同，位置也相同。

一份是另一份的收窄，指它在能枚举的两处取子集，其余按相同比：`in` 的顶层 anyOf，每一支都在另一份的某一支里相同，没有 anyOf 的按相同比；附带的声明，写明的名字是另一份允许的之内：另一份写明的名字，或它的 additionalProperties 允许的名字；同名的叶子按同一条递归。required 不比：接收方多要一些，是它自己承诺得更严，不超过它持有的；少要一些，交出的一方仍要满足链上其他节的 required。只收这两处，不引入别的包含判断：中间通配这类等价于正则语言的包含，判不准，也就不该写进通道能强制的地方。收窄用在接收方写声明的地方，见 ../Reference.md 的「获得」；那里放宽，现在能过的以后仍能过，结果记在 Journal 里，重放不重算。

- **描述**写它的功能：承诺做什么。描述是作者的说法，随 Definition 冻结。做得怎样是 Competence，不在描述里：它是引擎、Implementation、接上的 Reference、经历组合起来运行时的表现，单看 Definition 看不出。实例化的一方对它的把握来自旁边能检查的事实：Required port、结构属性、过往结果，以及 Instance 接上了什么。
- **in**：交给它的 Value 的形状。
- **附带的声明**：交给它时，能随 Value 带上哪些 Reference。写法与 value.schema 里对象的写法相同，只是叶子是所指 Port 的 Port spec：properties 写明名字与各自的 Port spec，required 列出必须带的，additionalProperties 管其余的名字：false 是不接受，给一份 Port spec 是名字任意、形状统一，如聚合者收 N 个投票者的 Port，true 是接受任意。叶子写 true 表示这个名字接受任意 Port。additionalProperties 必须写出，没有默认：以后补一个默认是增量，现在定了默认，以后再改就改变了已冻结的 Definition 的意思。additionalProperties 不是 true、叶子也没有 true 的，构造时算得出它能拿到哪些 Reference 的上限。写在一个位置上的声明就是对经过这里的 Reference 的收窄，规则在 Reference。
- **out**：提供者在这一步给出结果的 Port 才有，如 Kernel 的 Port、外部服务。Provided port 没有 out：交给 Instance 的只被记下，什么时候处理、回不回，由它的 Definition 决定。要回复，就声明回话途径：附带给对方、供它交回回复的 Reference，所指 Port 的 in 是回复的形状。它可以指向自己，也可以指向第三方。连接不默认双向：默认双向表达不了“只给一个方向”；要双向就两边都给。一问一答、多轮、反问、推送都是附带的嵌套，是扩展的用法。

Provided port 是它对外提供的服务。不同的 Provided port 是不同的权利，可以交给不同的一方，如一项服务的使用与管理。Required port 接上什么由别人决定：实例化时接上 Reference，运行中由持有 Correct 的一方授予、收回。读取、暂停、授予这些 Control，对所有主体都一样，作者不能选择"不被暂停"，所以不写进 Interface。Control 是 Kernel 提供的三个 Port：Look、Notify、Correct（JSON 里是 look、notify、correct）。名字定在这里，Charter 才能接；有哪些操作在 Instance，形状在 Port。

设计 Interface 时信任对方的 Competence：不假设对方会误解意图，不在 Interface 里设防。对方真的做不到，是 Competence 不匹配，应换一个更合适的。同质的主体犯相关的错，这类错由独立的检验去防，不靠 Interface。


## 完整身份与构造

正文和显式依赖组成一个 Bundle。构造只读该闭包，固定代码、被 pin 的 Definition 与输入所需内容；Kernel 不假设任何目录布局，body 的依赖如何解释由 Language 决定。

construct 成功的记录建立关于完整 Bundle 的正面事实，按 ID 索引，全系统可复用。检查只依赖固定对象、固定构造规则和已成立的依赖 Definition 事实，不能借构造者额外材料。登记 Language 的同一 ID 固定语义与检查规则；宿主临时保护不属于检查结论。

实例化先查本次已交付的完整根，再查构造事实、输入与接线。未取得对象时不泄露它是否已构造。同形宿主服务可以给出内容，但不能建立 Kernel 的构造事实；已经真实构造过的对象可以被后来的持有者复用。root entry 的 Definition 由宿主按相同检查验证，诞生固定根并建立启动事实，不隐式给予构造 Port。

案例：A 只把正文给 B，B 读不到缺失的代码；A 给出完整 Definition Bundle，B 可以复用已成立的构造事实，仍需 instantiate 和接线。正文相同、依赖改变产生新身份，必须重新构造。难改的点是完整身份与成功事实的依据。

## Cognition 与 Charter

两者共用 Interface，Seat 可以由 Individual 或 Collective 占据。区别在保证怎样兑现：Cognition 把求值、权利与历史放在一个行动身份上；Charter 把多个身份之间的权利与 Control 接线固定为结构，由 Kernel 按「刚性」强制。同一种描述语言可以表达两者，身份之间的分权仍要由 Kernel 兑现。

例如，执行者接上模型和 exec，检验者接上执行者的 Look 与只能暂停的 Correct；两边的 Port 都不接收附带。写成 Charter，执行者够不到检验者的权利，也不能暂停它；持有整体 Correct 的一方也只能按「刚性」改变接线。若改成同一个 body 里的两份上下文，它们用的是同一份 c-list，函数间的限制由 Language 与程序自身承担；独立的权利边界随 Instance 建立。

复用代码靠固定内容依赖与 Language 的模块机制：函数没有自己的 Interface，用调用方的权利，Atom 归调用方。复用主体靠实例化，有自己的身份、权利与 Journal。要约束被复用的一方，就实例化一个接线更窄的 Instance。

没有采用：

- 在一次求值里调用另一份 Cognition：它与 import 函数承担同样的保证；又等于一次求值里临时换了一份 Definition，而 Instance 不换 Definition；被调用者的 Interface 也没有人拿到。
- 只有一个 Seat 的 Charter：与直接实例化那份 Definition 承担同样的保证。要钉死 input 或收窄 Interface，写一份新的 Cognition，import 同一套函数。构造时拒绝；以后有场景再放宽是增量。
- 用一个编排者代替 Charter 的分权：编排者实例化全部成员、持有全部 Control，结构依赖它的代码按约定使用权利。需要固定的分权关系用 Charter；运行中决定的组建由 Instance 实例化。
- 比身份轻的权利边界，如 body 里的一段：段只有 Language 知道，按段分 c-list，保证就从 Kernel 降到 Language；给段独立的身份，它就是成员。

一个主体承担 Definition，Kernel 查形状、接线与每次交出；做法能否完成功能，由选定它的 Principal 判断。独立检验能观察什么、纠正什么，由 Reference 分配；模型和上下文怎样配备属于具体做法。


## Charter

Charter 只声明结构，自身没有行为：有哪些 Seat，每个 Seat 的 Required port 接到哪里，Collective 的 Signature 与 Provided port 接给哪个 Seat。行为都在占着 Seat 的成员里，聚合也由一个成员执行。外界按 Collective 的 Interface 与它交互。Charter 是一张带洞的接线图：Seat 是有类型的洞，填洞就是组合。

### Seat

Seat 以 Interface 要求成员，描述就是职责。占它的 Definition 要有同样的 Signature，提供 Seat 的每个 Provided port，需要的与 Seat 的 Required port 相同。比较的都是 Port spec，要求相同。

Seat 可以用 pin 写死 Definition，连同它的 Signature 的 in；没有写死的，在实例化时注入。需要归 Charter 固定的规则，如计票、房间、闸门、熔断，写在被 pin 的 Definition 里；允许 Principal 选择的做法，如评审与工人，留给实例化。pin 固定选中的 Definition 与输入；规则怎样落实，须检查那份 Definition。Entry 的外部程序不在 Definition 里，pin 它也固定不了系统外的做法。

成员在 Collective 的一生里固定，Kernel 不换人。能换，"Seat 现在由谁占"就是可变状态，要有一本 Journal 解释它，而 Collective 没有行为；新人接哪些工作、旧人做完哪些、什么时候退下，是迁移的策略，Kernel 不预设。要换的，是整个 Collective，或某个成员委托出去的 Agent。历史发生在主体身上：一个 Seat 上、一件任务里发生过什么，都从 Journal 投影出来。

成员是结构保证存在的，在 Seat 上；参与者是任何持有某个成员的 Port 的一方，可以在运行中经附带加入，如群聊中途进来的人。已有的 Instance 或外部服务不占 Seat：它们由别处产生，不随 Collective 结束，只能接到 Collective 的 Required port 上，供成员使用。

### 接线与导出

每个 Seat 的每个 Required port 恰好接一种来源：

| 来源 | 例子 |
|---|---|
| Collective 的一个 Required port | 外面接进来的引擎、exec、已有的服务 |
| 某个 Seat 的一个 Provided port | 评审把意见交给聚合者 |
| 某个 Seat 的 Look、Notify 或 Correct | 熔断者接执行者的 Correct，它的 Required port 的 Port spec 只收暂停；审计者接执行者的 Look |
| 自己的一个 Provided port | 指向自己的 Reference，可附带给对方作回话途径 |

表里第三行的收窄写在接收一方的 Required port 上。Seat 的 Interface 与占它的 Definition 要求相同，所以这一节是 Charter 的作者写的收窄，不是成员自己挑的；成员手里那一份按「收窄」判定，见 ../Reference.md 的「获得」。收窄得比 Collective 的 Required port 更窄时，外层给的更宽的那几份仍在链上，每次交出合取，成员用不到。

Collective 的 Signature 接给一个 Seat：这个成员拿到 Collective 的输入，它的结果就是 Collective 的结果。所以只有它听到 Collective 的输入，其余成员要它转交。其余 Seat 的 Signature 的 in，写死的写在 Charter 里，注入的在实例化时给。Collective 的每个 Provided port 接到某个 Seat 的一个 Provided port。几个成员接到同一个 Port，各得一份派生的 Reference，提供者凭来路分得清是谁。按 Charter 在成员之间接上的派生没有撤销权，随 Collective 一起结束：有撤销权，持有它的一方就能在 Charter 之外改掉成员之间的接线。接到 Collective 的 Required port 的，撤销权照常在给出它的一方，撤销沿派生链传到成员。

成员之间的关系不是原生的类型，是这些字段的取值：谁持有谁的 Provided port，谁持有谁的 Control，Signature 与 Provided port 接给谁，谁和谁接到同一个来源。评审、表决、熔断都是它们的组合。Charter 里的上级，就是持有下级的 Provided port 与收窄过的 Control。问责不在其中：成员的来处都是 Collective，成员之间没有委托关系。

多方不是原语。群聊这类几方共用的东西是一个成员，规则写在它的 Definition 里，常常写死；对接到它的成员来说，它的 Port 就是工具。

Charter 是数据，结构在运行前就能查看、比较、检查。所以不能把结构写成"运行到某一步再实例化"：运行中才决定的组建，如动态的编排、招标，是一份 Cognition 去实例化，它也可以实例化一份 Charter。

### 刚性

Charter 用两层约束兑现刚性：形式用 Port 的形状与 Reference 的限定，附带的嵌套描述连接上的往来，不另设协议语言；内容与状态的规则，如次数、先后、两方都同意，由 pin 写死的保管者成员执行，规则怎样检查见「Seat」。改这些固定做法要重写 Charter，成员在内部做不到，持有 Collective 的 Correct 的一方也做不到：它只能换 Collective 自己的 Required port 上接的东西。成员之间的接线只按 Charter 写明的方式改变：一个成员的 Required port 上接的，只因两种事变，一是 Collective 的 Required port 被授予、收回，二是 Charter 把这个成员的 Correct 接给了另一个成员，由它授予、收回；此外只有失效。所以修改规则的门槛高于遵守规则，Charter 的 ID 在运行中一直是它的结构。

Charter 承诺的是接线与写死的规则，不承诺成员的状态。成员的状态可变，如一个成员维护的名单，不算改结构：每次变化记在它的 Journal 里；谁能改，就是谁够得到改它的 Port，由接线、导出与附带的声明决定，构造时算得出；依赖它的一方能否感知，看它有没有读取或 Notify 的途径。显式声明就是这些字段，不另设「可变」的字段：另设一个，同一个事实就写了两处，对不上时强制不了。Port 接受任意附带的，谁能改只知道上限不明。Collective 自己不持有状态：它没有 body，也没有自己的一步，几方的写入没有谁来排顺序、套用合并规则。要让一份状态的规则归 Charter，就把持有它的成员写死。

接线图只描述一开始谁能够到谁；运行中成员可以把 Reference 附带出去。只有成员的 Port 不接受任意附带，接线图上的性质在运行中才一直成立。

### 检查

| 何时 | 查什么 |
|---|---|
| 构造 | 至少两个 Seat；每个 Required port 恰好接一种来源，形状兼容；导出的形状兼容；写死的 Definition 满足它的 Seat；没有成员接上自己 Seat 的 Correct；Seat 里的 Definition 可以是 Charter，按内容寻址，所以没有环 |
| 实例化 | 人选覆盖每个没有写死的 Seat，满足 Interface；Collective 的 Required port 都接上了 |
| 运行中 | 每次交出照常按 Port 查；授予、收回与失效按「刚性」改变接线，成员固定 |

构造与实例化的检查，都在任何成员运行之前做完。审计者是否独立、敏感的东西是否分给了不同的成员，这类图性质是 Charter 数据上的纯函数，Kernel 不查：查哪些是策略。


## 形状

定义本身的形状与构造时的检查见 Shape.md。

## 待定

- 模型差异等接入条件：两个 Seat 的引擎必须是不同的模型，要先能读到 Reference 提供者的事实，见 ../Port.md 的待定「一个 Reference 对外透露提供者的哪些事实」。哪些条件由普通接线与检查器实现，哪些需要 Kernel 在实例化及 Correct 改接时强制，随场景定，不预留通用资格字段。场景：异质检验、陪审团。
- 第五种来源：Collective 自己的 Look 与 Notify 能不能接给成员。Correct 不能：纠正整体就纠正了成员自己，全组熔断用几份对成员的暂停组合出来。与它相关：导出只能导 Provided port，Principal 要单独纠正某个成员，得设一个管理 Seat 接上那个成员的 Correct，再导出它的 Port，多一跳；是否允许直接导出某个成员的 Correct。Principal 参与自己实例化的 Collective 要不要单独的机制，与这一条一起定。场景：嵌套、Principal 参与自己实例化的 Collective、熔断之后。
- 整份 Interface 是否也单独按内容寻址。Port spec 已经单独成一个 Value。没有场景：现在没有按 ID 引用一份 Interface 的地方。

参考：../../README.md（分类与关系）；../README.md（主体）；../Value.md（JSON、Schema、内容寻址）；Shape.md（形状、构造时检查）；Cognition.md（求值器在系统外）；../Port.md（待定）。
