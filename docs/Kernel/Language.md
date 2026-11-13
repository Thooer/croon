# Language

Language 是 Kernel 的一个插口：Cognition 的 body 用某一种 Language 写成，Kernel 按求值契约驱动它。Kernel 只依赖契约，不依赖任何一种 Language，所以 Journal 与重放不随 Language 改变。契约本身——会话、Atom、键、恢复与分歧、Kernel 一侧的检查——在 Definition/Cognition.md 的「求值契约」，一次会话往来的形状在 Definition/Session.md；本文讲插口：什么算一个 Language，它要守什么。

## 插口

当前 Language 由根登记，实现是可信的宿主代码。让它成为系统内可构造对象的边界见「待定」；可信的含义见 Plugin.md 的「可信」。

Kernel 的保证不依赖 Language 守住自己的规则：body 可能在运行中绕过它们，但每次交出都由 Kernel 按 c-list 与派生链查，绕过的损失不超出这个 Instance 自己的权利；破坏了确定性，恢复时报出重放分歧。所以会被绕过的 Language 每个 Instance 一个隔离单元，碰不到别的 Instance 与内容库；两个 Atom 之间的有界，在计数被绕过时由宿主一侧兜住。

- **ID** 是登记的名字加版本，如 `lang0@1`，不用哈希：语义大半写在散文里。发布之后只增加，不改语义；还有 Instance 在用，就不删。
- **写在 Definition 里**：换 Language 就是换 Definition，旧 Instance 按旧 Language 重放。每种 Language 决定自己的写法与语义；Kernel 依赖求值契约。
- **装哪些由根决定**：Language 的实现是 Plugin；新增一种按 README.md 的「放置」判断。

## 守什么

分两部分：一部分由求值契约推出，Kernel 依赖它；一部分由需要决定，Kernel 不检查也不依赖——不守的 Language 只是更差，并不破坏保证。这一节里的两条分界与这个区别是同一件事。

### 由求值契约推出

Language 不是通用编程语言，必须拥有的只有两样：

- **Atom 与键**：结果不能重算的步骤：交出、取；键用来在重放时匹配结果。
- **在 Atom 处等结果、恢复时重放**：Language 报出 Atom，等 Kernel 送回结果，最后给出 done；恢复时按已有结果重放到中断处。

其余（写法、表达式、求值器）尽量借现成的：编排的语言一路加功能，最后会长成一门更差的通用语言。Language 算出要做什么，接触外部世界经持有的 Reference。新写法可以由 Library 函数解释数据、由前端转换成已有 Language 的程序，或交给独立求值器。采用独立 Language，看是否需要自己的构造检查、Interface、固定引用与恢复边界；已有 Language 能否计算同样的东西，不单独决定这一选择。写法与参数怎样组织属于程序，模型提示也不因命名约定而成为 Kernel 语法。

### 由需要决定：一套名字、两种值

Library 的做法要求模型只面对工具、名字与结果，不接触 Handle。为此 Language 的值分两种，共用名字和全部构造：

- **数据**：就是 JSON，交出时成为 Value。
- **Ref**（引用值）：指向一个自己持有的 Reference，不透明。

三条规则：

1. **引用值不透明。** 没有 builtin 能把引用值变成字符串，也没有 builtin 能把字符串变成引用值；repr、编码成 JSON、字符串操作碰到引用值就失败，报错也不给出 Handle。所以模型看不到 Handle，只能在 Definition 建的表里选。
2. **引用值不进 Value。** 交出的值（包括交给 llm 的）和 done 里只要含有引用值，就以 type 失败，报错时说出违反的是这条规则；引用值只能经附带出去。同一个 body 里普通函数的参数与结果不过边界，可以带引用值。
3. **Required port 不能按数据找到**：只经源码里写出的名字够到，没有把字符串变成 Required port 的写法。所以构造时算得出哪些 Required port 可能被附带出去：只作为交出目标流到的，不算；作为参数传进函数的，看函数会不会把它附带出去；流到附带的位置的，按那个位置上的声明算；流向算不清的（如放进按数据取值的表），算进上限。

这三条由需要决定，所以 Kernel 不检查也不依赖：它判断不了一个字符串是不是 Handle。Kernel 的防线不变：Value 里写着的 Handle 不算数，权利看 c-list。一个把 Handle 漏成字符串的 Language 只是更差。

### 由需要决定：从 body 推出 Interface

Language 可以从 body 推出 Interface，省得在源码之外再写一份。这时构造要多查一条：Definition 里的 interface 与推出的一致。Kernel 仍只看 interface 的 JSON，所以它不检查这一条。

### 契约边界上的转换

Language 在契约边界上按 definition.eval 里的位置做转换，分三样：

- **自己 c-list 里的，变成引用值**：交出结果带来的 attach、derived 里的 revoker、到达的 attach。三处是同一个形状，带着所指 Port 的 Port spec，Language 凭它知道引用值的类型。
- **认出别人的标识，仍然是数据**：derived 里的 handle、到达的 via 与 sender、名册。这些不在自己的 c-list 里，用不了，只用来对上「这条回复是沿我给出的哪一份回来的」。
- **交出时**：目标是经接线进来的就是 Required port，是运行中拿到的引用值就是 Handle，Language 自己清楚是哪种，不需要 Kernel 去猜。

## 实现

程序的求值接口是 definition.eval，JSON 进、JSON 出。求值器的装载与调用另有一层约定：怎样定位实现入口、调用检查和求值、传递数据与故障。它把实现接到既有契约，不解释某一种 Language 的正文；具体求值器不必和 Kernel 同进程、同语言。

反方向只有两样：按已取得的 ID 读 Value，业务读取受求值位置限制，构造读取固定 Definition 闭包；调用 Kernel 的校验器，校验器只有这一份。检查器对正文的判断不授予权利，Kernel 的行动检查与隔离照常成立。

登记 Language 的检查分两层：正常返回通过或正文拒绝，拒绝仍是 construct 的 use 失败；装载、检查 trap 与宿主临时保护耗尽是宿主故障，沿 construct、分派与请求执行交回宿主，阻断服务的那次请求，不写成永久的 Atom 失败。Language 的 open 与会话的 resume 同样显式返回宿主故障；实例化与 definition.eval 的形状不变。内部会话报出的 construct 遇此故障只停构造者：保存那次请求，不再消费其结果；显式重开时以 Journal 重建，未记录的 construct 可以重做。外部 Entry 的 submit 将故障交回客户端，允许同键重报。

## 待定

### Language 不必可信

场景是 README.md 的「用别人的 Language」。A 构造求值器 L，B 得到它，按 L 检查并构造自己的 Definition D，运行与恢复仍用原来的 L，全程无需根另装一份求值器插件。这是元循环的验收：检查与求值的做法也有构造来源和记录。L 不成为 Instance；Atom 与 Reference 属于使用它的 Instance，求值内部状态由重放重建。保证、执行保证与隔离的代码仍由根提供。

系统内构造解决引入 Language 是否仍需根逐一登记，由这个场景决定；四条保证按「插口」与求值契约成立。需要独立 Language 的判据是 B 要依赖这份做法的构造与执行边界。一份 Cognition 可以用库函数解释 input 中的程序（推断），前端也可转换写法；能否自然给出所需的 Interface、构造时检查与固定解释方式，要按场景判断。外来的普通库代码仍按调用方的权利运行，需权利隔离时另开 Instance；它本身不构成新增 Language 机制的理由。

候选是把实现固定为 Value，沙箱按既有契约运行它，没有环境 I/O、时钟与随机数，内容只经受控读取进入，指令语义确定，超出兜底上限时宿主拿回控制。根登记与系统内构造可作为同一 Language 的两种提供途径；登记求值器的隔离已接入，以下形状与机制仍待独立场景推进。

- **形状与调用**：独立 Value、Definition 的新变体与复用 Cognition 都是候选。实现要能定位检查与求值入口；code Tree 只确定文件，入口须由固定协议或显式声明确定。若 L 再写 language 指向解释它的另一份求值器，每一层都必须提供装载与调用能力，递归字段本身不能推出它。协议可由运行器身份确定，不重复设字段；宿主提供装载与隔离的起点，固定协议只覆盖能按它装载的实现。登记名固定语义，内容 ID 固定具体实现；实现字节改变即 L2，不要求证明语义是否相同。
- **身份与读取**：实现、依赖与影响行为的执行条件须由 D 的不可变依赖覆盖。构造 L 按 A，使用 L 检查 D 按 B，运行按使用它的 Instance 的读取资格；L 不继承 A 的其他权利。同一份实现因输入与读取环境不同而结果不同，不破坏身份。增加「作为 Language」的读法还是复用 Definition 的读法，与形状一起定；按内容 ID 能读不应省掉构造检查。已有登记名保持原语义；definition.language 已是字符串，新增内容 ID 主要改变解析、读取与装载规则。
- **执行条件**：Language 的 limit、固定执行上限与宿主临时兜底分开。固定条件可先随 L 声明；若以后允许 D 选择参数，参数与允许它的规则一起固定，不以 L 的 ID 不变否定这种用法。计量单位、作用范围、重置位置与引擎语义要明确；宿主给不了就在构造时拒绝，恢复时给不了按宿主故障停下，过去的构造结果不变。墙钟等临时兜底不进 Journal。与 exec 环境声明有无共同结构，等两边需要明确后再定。
- **检查与记录**：Kernel 检查对象形状、依赖可读与宿主条件，L 检查 D 的正文；检查本身要兜底，失败记在构造者的那次交出，已记录的结果重放不重算。D 的诞生经 Definition 固定 L，目前没有场景要求另记装载成功；查到原 ID 也不代替宿主实际兑现执行条件。
- **故障与恢复**：登记 Language 的故障按 Instance.md 的「执行」；内容钉住的 L 要改实现就得到 L2、换 D。后者耗尽声明上限，或违反求值契约，候选是 Kernel 结束 Instance，kind 为 terminated，reason 分别为 fuel、memory 或 contract；前两者依据执行条件，后者依据契约。重放前缀不应因恢复而挤占继续执行的额度；已走过的位置出错归宿主，怎样识别待定，guest 按键读取已有结果只是候选。终止原因、在途 Atom 收尾与事件定位随通用终止机制一起定，不能另造一套；以结束为原因、by 为 self、收尾共享事件仍须检验重建相容性。

装载已验证（bef8c78）：WASM 实现与固定执行条件按内容寻址，A 经 construct 固定 L，独立 B 构造并实例化引用 L 的 D；检查按 B、运行按 D 的读取资格，L2 不改变旧 D，该场景提交前后中断恢复得到相同完整 Journal。实验复用真实 Kernel，以五处源码接缝补上按 ID 装载、Language 依赖闭包、会话错误返回及临时包装的实例化限制；内容库与构造记录够用，没有新增注册表。包装暂借 Cognition 并禁止实例化，只是候选；B 保持等待，未验证任意结束调度。WASM 故障能交回宿主，终止记录、在途收尾、跨引擎确定性及每次从头求值的计量分界尚未验证。重跑用 git worktree add 到该提交，执行 spike/language-value/run.sh。

正式化时复用这条主线，再查读不到的实现与依赖、构造检查耗尽、运行中违约与终止收尾。难改的是 ID 的解释、读法、执行条件与故障记录；形状与机制随独立场景收敛，不因实验可行就提前冻结。

参考：Definition/Cognition.md（求值契约、Atom、失败、求值器在系统外）；Definition/README.md（Interface、Signature、Port）；Definition/Shape.md（形状、构造时检查）；Journal.md（记录、恢复、待定）；Instance.md（执行）；Reference.md（能读的 ID、待定）；README.md（放置、场景、待定）；Plugin.md（可信）。
