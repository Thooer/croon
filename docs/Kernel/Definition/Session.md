# 会话

Kernel 与 Language 之间是一次会话。开始时给出诞生和已有 Atom 结果；之后 Language 报新 Atom，Kernel 送回结果，直到 done。恢复时重新打开会话，结果按键匹配；输出不得依赖结果送回的先后。具体保证见 Cognition.md。

诞生固定完整 Definition Bundle、输入交付、Seat 与名册；不含默认业务状态。名册只列接到该成员 Provided port 的 Seat。Definition 内容随诞生给出，检查和代码装载只读它的固定闭包；运行中的业务读取按程序实际消费的结果推进资格。

宿主调用的错误返回独立于 JSON；open、resume、读取或宿主预算故障都停下求值，不伪造 body 的结果。某次结果已经提交、尚未送回时保留原值，恢复不重新执行。

进来的 Reference 携带 Handle 与完整 Port spec。规格本身是数据，交给 Language 判断本地类型，不从规格猜提供者。Handle 只按该 Instance 的 c-list 解析。派生链的真实规格与位置由 Kernel 保存。

```json definition.eval
{
  "in": {
    "type": "object",
    "properties": {
      "birth": {
        "type": "object",
        "properties": {
          "definition": { "description": "完整 Definition 的 JSON 内容" },
          "bundle": { "type": "string", "description": "Definition Bundle ID" },
          "input": { "$block": "value.delivery" },
          "seat": {
            "type": "object",
            "properties": { "name": { "type": "string" }, "description": { "type": "string" } },
            "required": ["name", "description"], "additionalProperties": false
          },
          "roster": { "type": "object", "additionalProperties": { "type": "string" }, "description": "相邻 Seat → Instance" }
        },
        "required": ["definition", "bundle", "input"], "additionalProperties": false
      },
      "results": { "type": "object", "additionalProperties": { "$ref": "#/$defs/result" }, "description": "键 → 结果" }
    },
    "required": ["birth", "results"], "additionalProperties": false,
    "$defs": {
      "incoming": {
        "type": "object",
        "properties": {
          "handle": { "type": "string" },
          "spec": { "type": "object", "description": "完整 Port spec，形状见 definition.schema 的 portspec" }
        },
        "required": ["handle", "spec"], "additionalProperties": false
      },
      "result": { "anyOf": [{ "$ref": "#/$defs/send_result" }, { "$ref": "#/$defs/receive_result" }] },
      "send_result": {
        "type": "object",
        "properties": {
          "outcome": { "$block": "definition.outcome" },
          "attach": { "type": "object", "additionalProperties": { "$ref": "#/$defs/incoming" } },
          "derived": {
            "type": "object",
            "additionalProperties": {
              "type": "object",
              "properties": { "handle": { "type": "string" }, "revoker": { "$ref": "#/$defs/incoming" } },
              "required": ["handle", "revoker"], "additionalProperties": false
            }
          },
          "accepted": { "type": "boolean", "description": "请求依赖和附带通过检查，已由实际提供者接下；业务失败也可为 true" }
        },
        "required": ["outcome", "attach", "derived", "accepted"], "additionalProperties": false
      },
      "receive_result": {
        "type": "object",
        "properties": { "arrival": { "anyOf": [{ "type": "null" }, { "$ref": "#/$defs/arrival" }] } },
        "required": ["arrival"], "additionalProperties": false
      },
      "arrival": {
        "type": "object",
        "properties": {
          "port": { "type": "string" }, "sender": { "type": "string" }, "via": { "type": "string" },
          "delivery": { "$block": "value.delivery" },
          "attach": { "type": "object", "additionalProperties": { "$ref": "#/$defs/incoming" } }
        },
        "required": ["port", "sender", "via", "delivery", "attach"], "additionalProperties": false
      }
    }
  },
  "out": {
    "anyOf": [
      { "type": "object", "properties": { "atoms": { "type": "array", "items": { "$ref": "#/$defs/atom" } } }, "required": ["atoms"], "additionalProperties": false },
      { "type": "object", "properties": { "done": { "$block": "definition.outcome" } }, "required": ["done"], "additionalProperties": false }
    ],
    "$defs": {
      "target": {
        "anyOf": [
          { "type": "object", "properties": { "port": { "type": "string" } }, "required": ["port"], "additionalProperties": false },
          { "type": "object", "properties": { "handle": { "type": "string" } }, "required": ["handle"], "additionalProperties": false }
        ]
      },
      "atom": { "anyOf": [{ "$ref": "#/$defs/send" }, { "$ref": "#/$defs/receive" }] },
      "send": {
        "type": "object",
        "properties": {
          "key": { "type": "string" },
          "send": {
            "type": "object",
            "properties": {
              "to": { "$ref": "#/$defs/target" },
              "delivery": { "$block": "value.delivery" },
              "attach": { "type": "object", "additionalProperties": { "$ref": "#/$defs/target" } }
            },
            "required": ["to", "delivery", "attach"], "additionalProperties": false
          }
        },
        "required": ["key", "send"], "additionalProperties": false
      },
      "receive": {
        "type": "object",
        "properties": {
          "key": { "type": "string" },
          "receive": {
            "type": "object",
            "properties": {
              "ports": { "type": "array", "items": { "type": "string" } },
              "sender": { "type": "array", "items": { "type": "string" } },
              "via": { "type": "array", "items": { "type": "string" } },
              "timeout": { "type": "integer", "description": "秒，0 不等待；不给一直等" }
            },
            "required": ["ports"], "additionalProperties": false
          }
        },
        "required": ["key", "receive"], "additionalProperties": false
      }
    }
  }
}
```

```json definition.outcome
{
  "anyOf": [
    {
      "type": "object",
      "properties": { "ok": { "enum": [true] }, "value": {}, "dependencies": { "$block": "value.dependencies" } },
      "required": ["ok", "value", "dependencies"], "additionalProperties": false
    },
    {
      "type": "object",
      "properties": { "ok": { "enum": [false] }, "error": { "$block": "definition.failure" }, "dependencies": { "$block": "value.dependencies" } },
      "required": ["ok", "error", "dependencies"], "additionalProperties": false
    }
  ]
}
```

outcome.dependencies 是实际交付的根，成功、失败都能有产物；它不受 accepted 的真假推导。accepted 只解释请求是否接下，不能授予结果里未出现的根。宿主输出违反 out 时，业务值改为 use 失败，已成立的产物仍在失败结果的 dependencies 中交付；候选诞生、派生和到达不提交。

案例：A 请求执行 R0，命令失败但产生 R1，accepted 为 true，失败结果交付 R1；A 请求一个没有读取资格的根，accepted 为 false、结果依赖为空。重建不用从失败 kind 猜接受事实。难改的点是结果的资格与请求位置，权威见 Reference。

参考：Cognition.md（求值契约、失败）；../Value.md（Bundle、交付）；../Reference.md（能读的 ID）。
