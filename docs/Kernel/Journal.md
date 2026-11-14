# Journal

Journal 记下一个 Instance 发生过什么，只追加；当前状态由它投影。body 的函数和 import 代码都属于这次求值，没有自己的 Journal。模型上下文是某次交给模型的 Value，历史与上下文视图由程序和扩展投影。

## 记录

每条记录有从 0 起的 seq 和宿主写下的 at。by 指造成记录的一步所在的 Instance，本 Instance 写 self，系统外写 outside。Kernel 执行规则，不作为新的行动者。自己的 Atom 有重放不变的 key；同一个键只对应一条结果记录。

别人造成的记录带 event。同一次事件在某本 Journal 里恰好有一条造成它的记录，其余记录共享 ID；root entry 的诞生例外，由 outside 造成。一次事件在同一本 Journal 里的顺序是到达、失效、按 key 字典序排列的截断 Atom、结束。事件因果和请求位置分别由 event 与 key 表达。

record 中的 delivery 把 value 写成内容 ID，其他信息形状与会话一致。accepted 是决定阶段固定的事实；业务失败不能据此改写请求是否被接下。资格从记录确定，规则在 Reference 的「能读的 ID」。

```json instance.delivery
{
  "type":"object",
  "properties":{"value":{"type":"string","description":"Value ID"},"dependencies":{"$block":"value.dependencies"}},
  "required":["value","dependencies"],"additionalProperties":false
}
```

```json instance.record
{
  "type":"object",
  "properties":{
    "seq":{"type":"integer"},"at":{"type":"string"},"by":{"type":"string"},"event":{"type":"string"},
    "what":{
      "anyOf":[
        {"type":"object","properties":{"send":{"$ref":"#/$defs/send"}},"required":["send"],"additionalProperties":false},
        {"type":"object","properties":{"receive":{"$ref":"#/$defs/receive"}},"required":["receive"],"additionalProperties":false},
        {"type":"object","properties":{"end":{"$block":"definition.outcome"}},"required":["end"],"additionalProperties":false},
        {"type":"object","properties":{"birth":{"$ref":"#/$defs/birth"}},"required":["birth"],"additionalProperties":false},
        {"type":"object","properties":{"arrival":{"$ref":"#/$defs/arrival"}},"required":["arrival"],"additionalProperties":false},
        {"type":"object","properties":{"correct":{"$ref":"#/$defs/correct"}},"required":["correct"],"additionalProperties":false},
        {"type":"object","properties":{"invalid":{"type":"array","items":{"type":"string"}}},"required":["invalid"],"additionalProperties":false}
      ]
    }
  },
  "required":["seq","at","by","what"],"additionalProperties":false,
  "$defs":{
    "send":{
      "type":"object",
      "properties":{
        "key":{"type":"string"},"to":{"$block":"definition.eval#/out/$defs/target"},
        "ref":{"anyOf":[{"type":"string"},{"type":"null"}]},
        "delivery":{"$block":"instance.delivery"},
        "attach":{
          "type":"object",
          "additionalProperties":{
            "type":"object",
            "properties":{"to":{"$block":"definition.eval#/out/$defs/target"},"ref":{"anyOf":[{"type":"string"},{"type":"null"}]}},
            "required":["to","ref"],"additionalProperties":false
          }
        },
        "result":{"$block":"definition.eval#/in/$defs/send_result"}
      },
      "required":["key","to","ref","delivery","attach","result"],"additionalProperties":false
    },
    "receive":{
      "type":"object",
      "properties":{
        "key":{"type":"string"},"filter":{"$block":"definition.eval#/out/$defs/receive/properties/receive"},
        "result":{"anyOf":[{"type":"integer"},{"type":"null"}]}
      },
      "required":["key","filter","result"],"additionalProperties":false
    },
    "birth":{
      "type":"object",
      "properties":{
        "definition":{"type":"string","description":"完整 Definition Bundle ID"},
        "input":{"$block":"instance.delivery"},
        "wire":{"type":"object","additionalProperties":{"type":"string"}},
        "binding":{"$block":"reference.binding"},
        "self":{"type":"object","additionalProperties":{"type":"string"}},
        "collective":{"type":"string"},"seat":{"type":"string"},
        "roster":{"type":"object","additionalProperties":{"type":"string"}},
        "staffing":{
          "type":"object",
          "additionalProperties":{
            "type":"object",
            "properties":{"definition":{"type":"string"},"input":{"$block":"instance.delivery"}},
            "required":["definition","input"],"additionalProperties":false
          }
        }
      },
      "required":["definition","input","wire"],"additionalProperties":false
    },
    "arrival":{
      "type":"object",
      "properties":{
        "port":{"type":"string"},"sender":{"type":"string"},"via":{"type":"string"},
        "delivery":{"$block":"instance.delivery"},
        "attach":{"type":"object","additionalProperties":{"$block":"definition.eval#/in/$defs/incoming"}}
      },
      "required":["port","sender","via","delivery","attach"],"additionalProperties":false
    },
    "correct":{
      "anyOf":[
        {"$block":"reference.kernel#/correct/in/$defs/pause"},
        {"$block":"reference.kernel#/correct/in/$defs/resume"},
        {"type":"object","properties":{"op":{"enum":["grant"]},"port":{"type":"string"},"ref":{"type":"string"}},"required":["op","port","ref"],"additionalProperties":false},
        {"$block":"reference.kernel#/correct/in/$defs/withdraw"}
      ]
    }
  }
}
```

诞生必须是第一条；结束后不再追加。成员的 wire 只记 Handle：它接的东西从哪来是 Charter 的事，记在 Collective 的诞生与 Charter 的 Definition 里，见 ../Reference.md 的「限定」。root entry 的诞生必须有 binding、self，不能有 collective、seat、staffing。普通 Instance 的 self 来自实例化请求；成员有 collective、seat、roster；Collective 有 staffing。这些角色条件在写入决定时检查，不凭可选字段切换契约。

到达记录谁交的 sender、沿哪一份 via、由哪一步造成的 by。Notify 的 sender 是目标；已经结束后才登记的，by 是登记者，所以两者不能合并。

## 四条不变量

1. 同一个 key 只有一条结果记录，交出、取、失败和被拒都占用它。[test:crates/kernel/src/journal.rs:a_key_has_one_record_whether_it_is_a_send_or_a_receive]
2. 当前代际求值过的 Atom 都有记录；未有结果的 Atom 没有 seq。旧代际的请求被拒，不算这个 Instance 的一步。[test:crates/kernel/tests/kernel.rs:a_refused_send_and_a_timed_out_receive_are_recorded] [test:crates/kernel/tests/kernel.rs:an_old_session_cannot_write_and_leaves_nothing_behind]
3. 事件的造成记录和全部结果在同一事务；root entry 的诞生只由 outside 造成。[test:crates/scenarios/tests/expected.rs:events_line_up]
4. 一个 Instance 影响另一个，当且仅当它的一步在对方 Journal 里造成记录。Look、取和向 Notify 登记本身不改目标。[test:crates/kernel/tests/kernel.rs:reading_a_journal_leaves_no_record_and_gives_no_handles]

## 恢复就是重放

平时会话保持，崩溃后重新打开；已有结果按 key 返回，未完成的按执行恢复契约处理。记录请求与重放请求不同就停下，这是宿主故障；不能让 body 接住后在分歧状态继续。宿主停止不写成业务结束，Look 仍为 running 或 paused，Correct 仍可行动。

纯读取按 Language 分支实际消费的结果键决定，规则见 Reference。恢复不是把完整 Journal 当前的读取范围提前交给程序。

取固定一条满足 filter 的最早未取到达，结果写它的 seq；重放取到同一项，所以每项至多被取一次。暂停时到达仍可记，求值和取停下。进上下文则是 body 之后明确交给模型的另一项行动。

执行已经完成、结果尚未提交时保留完成对象。进程崩溃后先处理宿主执行与终止意图，再重开求值；没有结果不等于可以不经清理直接重做。业务位置依 Journal，物理执行恢复依持久占位。

## 实现

记录库本地 SQLite，WAL、synchronous=FULL；云上可换数据库。代际、停止标记、执行占位、终止意图是宿主恢复状态，不是业务事实，不进入业务索引。表结构只在代码中。

一次提交分三步：决定读索引、固定判断并产生全部记录；数据库事务检查代际、写全部记录并完成相关占位转换；Index::apply 更新内存。重建复用 apply，只记录已判定事实，不重新检查当时授权。第一版每个系统一次只决定、提交一个事务，在途执行不占着事务。

Value 先落盘，记录随后提交；内容写入幂等，未被提交结果引用的内容不授予任何资格。跨 Journal 事务是一个系统的事务；不同系统经服务交互。

Index::apply 保存记录自身的内容、固定 ID 与位置引用，查询时解开跨 Journal 关系；不读取全局提交顺序。成功 construct 的记录按固定目标与 Bundle ID 加入构造事实，root entry 的诞生按宿主已检查的 Definition 建立启动事实。只缓存成功，宿主条件可用性不冒充正文有效性。

案例：到达已提交、取未提交时恢复会取同一最早项；结果已提交但会话未收到时直接重放；被拒请求不会因重建时内容已存在而通过。难改的点是每种记录的取得集合和因果归属，均在决定中固定，不留重建时的猜测。

## 待定

- Checkpoint 缩短长 Journal 的重放；状态和计数器一起固定，随 Language 需要引入。
- 从 Journal 前缀开新 Instance 只在需要调试原做法时考虑；新的 Definition 直接用显式输入和依赖试跑。
- 出站效果的 unknown、对账及补全记录随联网场景定，当前隔离执行不联网。
- 跨信任边界的防篡改需要哈希链等独立机制，Journal 只追加本身不抵抗宿主修改。

参考：Reference.md（能读的 ID、通道）；Instance.md（执行、产生与结束）；Definition/Cognition.md（求值契约、失败）；Definition/Session.md（会话）；Value.md（Bundle）。
