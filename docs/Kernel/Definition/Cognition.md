# Cognition

Cognition 是求值器在宿主的 Individual 的 Implementation：body 用某一种 Language 写成，由 Kernel 按求值契约驱动。本文讲求值契约、body 的边界与失败；Interface、Signature、Port 见 README.md。

## 求值契约

Language 的写法与语义由用途决定，如便于模型书写或表达审批规则。它满足的契约由保证推出：两个 Atom 之间的执行确定、有界；body 只经 Interface 与持有的 Reference 进出；每个 Atom 有重放时不变的键；只在 Atom 之间暂停。Kernel 只依赖契约，不依赖任何一种 Language，所以 Journal 与重放不随 Language 改变。

Language 是可信的宿主代码，Kernel 的保证不依赖它守住自己的规则。插口的性质、ID 与版本、它要守的规范，见 ../Language.md。

Atom 是结果不能重算的一步，只有两种：

- **交出**：沿 Required port 或 Handle 交付数据、显式依赖和 Reference 附带。交给 Kernel 或宿主服务的结果来自提供者；交给 Provided port 的结果表示到达已提交，不等待处理。回执给出实际结果、依赖、附带及每份派生的 Handle 与撤销权，没有领域产物的特殊字段。
- **取**：从自己的 Provided port 取一项到达。可以只取某几个 Port 的、某个来路的，可以等，可以不等，可以设上限。结果是取到的那项到达，或者为空。

会话与结果的形状见 Session.md。

Language 一侧：

- 输出只由输入决定。按 ID 读 Value 不破坏这一点，内容寻址，读到的永远相同；不读时间与随机数，不接触世界。
- 输出只依赖 results 这个集合，与结果到达的顺序无关，所以并行的 Atom 不用记交错的顺序。要依赖先后的，如几项到达谁先到，本身是一次取，先后记在它的结果里。
- 一个键只对应一个 Atom：同一个键给出同样的交出或取。
- 每个 Atom 只报出一次；已有结果的不再报出，Kernel 不重做。
- 无论 Definition 怎样写，求值都以 done 或确定的失败结束：两个 Atom 之间，Language 在有界的计算内报出下一个 Atom 或 done；资源用尽是一次失败，只取决于 Definition、输入与 Atom 的结果。量什么、上限多少由 Language 定。理由：Correct 在 Atom 之间生效，走不到下一个 Atom 的 Instance 就拒绝了暂停；崩溃不记录，恢复后重放到同一处还会再来。所以崩溃只来自宿主一侧：进程被杀，Kernel 或 Language 的 bug。Definition 不需要信任，写错了只会失败，见 ../README.md 的「放置」。

Kernel 一侧：同一个键在恢复后给出的 Atom 与记录不同，就是重放分歧，报告并停下。还没有结果的 Atom 没有记录，崩溃后就按通道重新执行，不需要记：重放把有结果的前缀确定地重算出来，停在第一个没有结果的 Atom，正是它该重新执行的地方。Language 报出的不合 definition.eval 的 out，或交出的值不是 Value（如超出 value.limits），与重放分歧一样报告并停下，不记：求值契约要它以 done 或确定的失败结束，报出这种东西是 Language 的 bug，修好它同一个 Instance 就能接着走，见 ../Instance.md 的「执行」；不记，记录的形状就不必放宽。还有报出的 Atom 没有结果时给出 done，同样处理：Language 不能用 done 替未完成的 Atom 收尾，Kernel 造成的结束才按 ../Instance.md 的「执行」补齐结果。这里描述登记的 Language；按内容寻址的不受信 Language 如何区分故障与结束，见 ../Language.md 的待定。求值器在系统外的，停下的是这次会话。交出的目标与附带都带着标记。标为 port 的，按那个 Required port 现在接上的 Reference 解析；标为 handle 的，按自己的 c-list 解析；解析不到或已被撤销的，被拒。值每经过一个 Port，就按所经的 Reference 现在允许的检查：交出的 Value 与附带按 in 与附带的声明，结果按 out；实例化给的输入按 Signature 的 in，结束时的结果按 Signature 的 out。不满足的，记在交出这个值的一方，种类见「失败」：交出不对是这个 Atom 失败，输入不对是这次实例化失败，结果不对是交出结果的主体以失败结束。所以 Interface 对每次交互都成立，这不依赖 Language 写得对。

### 求值器在系统外

求值器在系统外的 Definition 没有 Implementation，在 definition.schema 里写作 `evaluator: "external"` 加 Interface。它定义一个 Entry：Individual 的身份与 Journal 仍由 Kernel 管理，客户端负责报出 Atom、接收结果并保存自己的执行位置；root entry 是根的 Entry，见 ../README.md 的「根」。外部求值不由 Kernel 重放，因此 Language 的确定、有界要求不适用；是否由人、固定程序或模型驱动不改变 Kernel 的行动检查与记录。

- 暂停时，Kernel 不接受它新报出的 Atom，不用等它走到 Atom 之间：两个 Atom 之间，它碰不到系统里的任何东西。
- 它的状态在系统外，由它自己保存。重连就是重开一次会话，Kernel 不重放，也不必一次给出全部已有结果：用原来的键重报，有记录的直接得到结果，不会重做。同一个键报出不同的 Atom 是分歧，停下的是这次会话。
- 一个客户端连上就是取得执行权，代际号加一，旧会话的交出被拒，见 ../Instance.md 的「执行」。
- 它只经会话与按 ID 读 Value 碰到 Kernel，读到的只有这个 Instance 能读的。
- 实例化它时，Kernel 写下诞生，不起隔离单元；到达照常记下，但不把它标为待执行，它等客户端连上。谁能连上：第一版只有 root entry，由宿主创建，连接靠 socket 的文件权限；别的外部驱动怎样授权连上，见 ../README.md 的「系统边界」。

## body 的边界

Interface 就是 body 的边界，body 只经它与持有的 Reference 进出：

| | 入口 | 出口 |
|---|---|---|
| 信息 | 诞生；取到的 Value；Atom 的结果 | 交出的 Value；done |
| 权利 | Required port 的接线与授予；取到的附带；交出结果带来的 Reference | 交出时的附带 |

按 ID 读 Value 只是展开已有的信息，不算新的入口。这要求存储不是后门：Language 与 Kernel 的 Port 只按这个 Instance 经自己的 Interface 拿到的 ID 读。否则只要猜到或得知一个 ID，就读得到没经过自己 Interface 的东西，存储就成了一个内建的、谁都能读的共享空间。共享与保密本身由扩展组合；Kernel 只保证组合不会被这样绕开。怎样算拿到，见 Reference 的「能读的 ID」。

body 所需代码在完整 Definition Bundle 的显式依赖中。Language 解释 body 并定位代码，Kernel 不假设目录布局。构造和代码装载只读这个闭包，业务读取按消费过的结果推进；规则见 Reference 的「求值中的读取位置」。共享代码就是依赖同一不可变内容。

- 每个 Atom 等到自己的结果。交给 Instance 的,结果不等对方处理;对方要回应,就交回来,成为一项到达,再由一次取得到。Kernel 里没有“等另一个主体处理完”这样的原语。也不强制交出的一方等：先记下、由对方取，等与不等都组合得出；强制等，不等就写不出了。
- 取不经持有的 Reference，是对自己的 Provided port 的操作，不能转交：别人取走，就是在消耗它的到达。
- body 看不见 Control：它交出时用的是 Required port 的名字，那里接着什么，由持有 Correct 的一方改。

## 失败

```json definition.failure
{
  "type": "object",
  "properties": {
    "kind": { "type": "string", "description": "契约层的 use、denied、type、terminated，或 Language 定义的" },
    "message": { "type": "string" },
    "data": {}
  },
  "required": ["kind", "message", "data"],
  "additionalProperties": false
}
```

| kind | 何时 |
|---|---|
| use | Port 另一侧报告失败 |
| denied | 交出被拒：Reference 解析不到、已被撤销，或交给的 Instance 已经结束；或者值与附带满足自己这一侧写的 Port spec，却被别人加的收窄拦下 |
| type | 值不满足自己这一侧写的 Port spec（没写的，用提供者的）、Signature 的 Schema；或者附带不合接收方写的声明 |
| terminated | Kernel 结束了这个 Instance，或为此截断了它在途的交出，见 ../Instance.md 的「执行」。data 只写 Kernel 观察到的原因；所属的 Collective 结束时，reason 是 collective |

失败怎样沿结构传、能否接住，由 Language 定义。没有接住的，主体以失败结束。Kernel 造成的结束不交回 Language，terminated 的收尾结果也不让 body 接着求值。

## 待定

- 并行 Language 的消费轨迹与 Checkpoint 随独立并发场景验证；结果集合与纯读取语义已由契约固定，不引入 Kernel 的当前业务状态。

参考：README.md（Interface、Signature、Port、附带、Seat）；Shape.md（形状、构造时检查）；Session.md（会话）；../Value.md（Schema、Bundle、内容寻址）；../Language.md（插口、守什么）；../README.md（根、待定）；../Instance.md（执行）；../Journal.md（四条不变量）；../Reference.md（能读的 ID）。
