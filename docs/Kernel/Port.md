# Port

Kernel 提供通用内容操作、构造与实例化，以及作用在已有对象上的 Control 和撤销权。Reference 的来源、持有、派生和检查在 Reference.md。模型与文件执行是宿主服务，其规格由装配选用，Kernel 不按规格猜目标。

## Kernel 的 Port

pack 把内容和已取得的依赖建立为 Bundle。content 接受新 JSON、base64 新字节、已能读的 Value，或按顺序拼接的已有字节内容。解码与拼接后按 value.limits 检查。JSON 请求受 json_bytes 限制；较大的字节可分块 pack 后按 parts 拼接，结果受 raw_bytes 限制，不要求把全部字节塞进一个 JSON。dependencies 来自本次请求交付，必须覆盖新 Bundle 的依赖；已有内容与 parts 也只能从本次闭包中读取。Kernel 规范化集合，规则见 Reference 的「能读的 ID」。

construct 接收 Definition Bundle ID；对象必须在本次交付闭包里。它只检查该完整对象，成功记录建立全系统可复用的构造事实。实例化也只使用本次明确交付的 Definition 和输入，输入的 dependencies 必须包含于外层请求闭包。

instantiate 的 Reference 附带恰好覆盖目标 Definition 的 Required port，self 中覆盖的除外。结果附带的名字固定为 port/<Provided port 名字>、look、notify、correct；派生撤销权在共同回执的 derived 中。correct 的 grant 恰好带 ref，按目标 Required port 的声明检查。动态名字由 Kernel 解释，下面形状只给通用上限。

成功结果按所经各节 out 检查业务值、附带名字和实际规格。附带的具体叶子必须与实际 Reference 的完整 Port spec 及位置相同，比较忽略 description，true 不限，判据见 Definition 的「Port」。结果附带不凭空增加收窄节；不满足时以 use 失败，候选诞生、到达和派生不提交；实际执行产物按会话结果规则保留。

notify 接下 to 时查它链上每一节能收 definition.outcome：in 是空 Schema、等于它，或 anyOf 有一支等于它；比较忽略各处 description。登记之后目标结束，或目标已结束，就把原结束内容和实际依赖交到 to。to 只用一次；撤销派生出的那份就是退订。

```json reference.kernel
{
  "pack": {
    "description": "把内容和本次交付的依赖建立为 Bundle，返回 ID 并交付该根",
    "in": {
      "type": "object",
      "properties": {
        "content": {
          "anyOf": [
            {"type":"object","properties":{"json":{}},"required":["json"],"additionalProperties":false},
            {"type":"object","properties":{"base64":{"type":"string"}},"required":["base64"],"additionalProperties":false},
            {"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false},
            {"type":"object","properties":{"parts":{"type":"array","items":{"type":"string"}}},"required":["parts"],"additionalProperties":false}
          ]
        },
        "dependencies": {"$block":"value.dependencies"}
      },
      "required":["content","dependencies"],"additionalProperties":false
    },
    "attach":{"additionalProperties":false},
    "out":{"value":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false},"attach":{"additionalProperties":false}}
  },
  "construct": {
    "description":"检查完整 Definition Bundle；成功事实属于该对象",
    "in":{"type":"object","properties":{"definition":{"type":"string"}},"required":["definition"],"additionalProperties":false},
    "attach":{"additionalProperties":false},
    "out":{"value":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false},"attach":{"additionalProperties":false}}
  },
  "instantiate": {
    "description":"实例化已构造的 Definition，输入依赖来自本次交付；Required port 从附带或 self 接上",
    "in": {
      "type":"object",
      "properties":{
        "definition":{"type":"string"},
        "input":{"$block":"value.delivery"},
        "seats":{
          "type":"object",
          "additionalProperties":{
            "type":"object",
            "properties":{"definition":{"type":"string"},"input":{"$block":"value.delivery"}},
            "required":["definition"],"additionalProperties":false
          }
        },
        "self":{"type":"object","additionalProperties":{"type":"string"},"description":"Required port → port/<自己Provided port> 或 look"}
      },
      "required":["definition","input","seats","self"],"additionalProperties":false
    },
    "attach":{"additionalProperties":true},
    "out":{"value":{"type":"null"},"attach":{"additionalProperties":true}}
  },
  "look": {
    "description":"读取目标状态或 Journal；Handle 只是信息，依赖按被读位置实际取得的范围交付",
    "in":{
      "anyOf":[
        {"type":"object","properties":{"op":{"enum":["inspect"]},"seat":{"type":"string"}},"required":["op"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"enum":["read"]},"seat":{"type":"string"},"from":{"type":"integer"},"limit":{"type":"integer"}},"required":["op"],"additionalProperties":false}
      ]
    },
    "attach":{"additionalProperties":false},
    "out":{"value":{},"attach":{"additionalProperties":false}}
  },
  "notify": {
    "description":"沿附带的 to 交付一次结束结果；带 seat 时作用在对应成员",
    "in":{"type":"object","properties":{"seat":{"type":"string"}},"additionalProperties":false},
    "attach":{"properties":{"to":true},"required":["to"],"additionalProperties":false},
    "out":{"value":{"type":"null"},"attach":{"additionalProperties":false}}
  },
  "correct": {
    "description":"纠正目标；Collective 只作用在整体，没有 seat",
    "in":{
      "anyOf":[{"$ref":"#/$defs/pause"},{"$ref":"#/$defs/resume"},{"$ref":"#/$defs/grant"},{"$ref":"#/$defs/withdraw"}],
      "$defs":{
        "pause":{"type":"object","properties":{"op":{"enum":["pause"]}},"required":["op"],"additionalProperties":false},
        "resume":{"type":"object","properties":{"op":{"enum":["resume"]}},"required":["op"],"additionalProperties":false},
        "grant":{"type":"object","properties":{"op":{"enum":["grant"]},"port":{"type":"string"}},"required":["op","port"],"additionalProperties":false},
        "withdraw":{"type":"object","properties":{"op":{"enum":["withdraw"]},"port":{"type":"string"}},"required":["op","port"],"additionalProperties":false}
      }
    },
    "attach":{"properties":{"ref":true},"additionalProperties":false},
    "out":{"value":{"type":"null"},"attach":{"additionalProperties":false}}
  },
  "revoke": {
    "description":"沿派生链撤销这次给出的 Reference",
    "in":{"type":"null"},"attach":{"additionalProperties":false},
    "out":{"value":{"type":"null"},"attach":{"additionalProperties":false}}
  }
}
```

Look 的结果使用以下形状；单列形状避免把读取的记录 Schema 嵌套回 Port spec、再经 Definition 回到记录而形成规格展开环。Look 的 out 接受 Value，Kernel 产生的结果另外按本 block 校验。

```json reference.look.result
{
  "anyOf":[
    {"type":"object","properties":{"status":{"enum":["running","paused","ended"]},"outcome":{"anyOf":[{"$block":"definition.outcome"},{"type":"null"}]}},"required":["status","outcome"],"additionalProperties":false},
    {"type":"object","properties":{"records":{"type":"array","items":{"$block":"instance.record"}}},"required":["records"],"additionalProperties":false}
  ]
}
```

## 根绑定

root entry 诞生必须明确给出 binding。sources 恰好覆盖 self 以外的 Required port；每项固定 Kernel 操作或宿主源与实际 Port spec。wire 的 Handle 指认来源；宿主按来源和明确的提供者恢复身份装配实现。

来源规格与接收方声明按相同规则比较，description 可以不同，派生链分别保存各自准确的 ID 与位置。self Look 使用共同 Look，无需为根设另一份契约。缺项拒绝，来源不由名称或同形规格推断。

```json reference.binding
{
  "type":"object",
  "properties":{
    "sources":{
      "type":"object",
      "additionalProperties":{
        "anyOf":[
          {"type":"object","properties":{"kernel":{"enum":["pack","construct","instantiate"]},"spec":{"type":"object"}},"required":["kernel","spec"],"additionalProperties":false},
          {"type":"object","properties":{"host":{"type":"string","description":"宿主恢复身份"},"spec":{"type":"object"}},"required":["host","spec"],"additionalProperties":false}
        ]
      }
    }
  },
  "required":["sources"],"additionalProperties":false
}
```

案例：同形宿主检查服务和 Kernel construct 接到不同来源，前者可以交付内容，只有后者建立构造事实。pack 取得新根后才能交给别方；失败请求声明的根没有这种效果。难改的点是所指、规格和交付来源的区分，索引重建按记录保持它们。

## 待定

- Reference 对外给出模型、窗口、tokenizer 与执行环境等事实，供异质检验和预算使用；放在哪项服务中，随场景决定。
- Look 目前按 from 与 limit 读记录；按时间、事件或投影查询等需要出现再加。
- 模型的 provider 消息、推理签名、部分结果、usage 与缓存控制，由模型服务的具体契约收敛；通用交付不解释它们。

参考：Reference.md（获得、限定、能读的 ID、通道）；Definition/README.md（Port、附带、Seat、刚性）；Definition/Session.md（会话）；Journal.md（记录）。
