# 无模型审批

根经一个 Entry 交出两类意见。审批者没有 Required port，也不收附带：分别从 left、right 取一项布尔意见，两项都同意才批准。根持有它的两个意见 Port、Look、Notify 与 Correct；向 Notify 登记之后交出第一项意见，等它取到，再暂停它。第二项意见在暂停期间到达，重启后仍保留，解除暂停后才被取。这里第二项是否决，所以决议为不批准。

这个场景检验单 Entry、固定审批规则、Control、通知与恢复，不把两类意见解释成两个已认证的参与者。Definition 的 fake@1 供共用脚手架使用；独立的审批求值器按同一规则计算结果，不从预期 Journal 生成程序。测试装配使用固定系统密钥，使同一次取得的两个 Port 失效时的 Handle 顺序也有固定预期。

```json scenario.approval
{
  "values": {
    "sha256:approvalroot": {
      "$bundle": {
        "evaluator": "external",
        "interface": {
          "description": "审批的 root entry",
          "signature": {"in": {},"out": {}},
          "provides": {"done": {"$block": "scenario.reconnect#/values/sha256:psdone"}},
          "requires": {"construct": {"$block": "reference.kernel#/construct"},"instantiate": {"$block": "reference.kernel#/instantiate"},"back": {"$block": "scenario.reconnect#/values/sha256:psdone"}}
        }
      },
      "dependencies": []
    },
    "sha256:opinion": {"$block": "scenario.approval#/parts/opinion"},
    "sha256:approval": {
      "$bundle": {
        "language": "fake@1",
        "interface": {
          "description": "两类意见都同意才批准",
          "signature": {"in": {"type": "null"},"out": {"type": "object","properties": {"approved": {"type": "boolean"}},"required": ["approved"],"additionalProperties": false}},
          "provides": {"left": {"$block": "scenario.approval#/parts/opinion"},"right": {"$block": "scenario.approval#/parts/opinion"}},
          "requires": {}
        },
        "body": "approval"
      },
      "dependencies": []
    },
    "sha256:constructapproval": {"definition": "sha256:approval"},
    "sha256:spawnapproval": {"definition": "sha256:approval","input": {"value": null,"dependencies": []},"seats": {},"self": {}},
    "sha256:agree": true,
    "sha256:disagree": false,
    "sha256:approvalout": {"ok": true,"value": {"approved": false},"dependencies": []}
  },
  "parts": {"opinion": {"description": "交出一项意见","in": {"type": "boolean"},"attach": {"additionalProperties": false}}},
  "journals": {
    "i-root": [
      {
        "by": "outside",
        "what": {
          "birth": {
            "definition": "sha256:approvalroot",
            "wire": {"construct": "h-rconstruct","instantiate": "h-rinst","back": "h-rback"},
            "self": {"back": "port/done"},
            "input": {"value": "sha256:null","dependencies": ["sha256:approval"]},
            "binding": {"sources": {"construct": {"kernel": "construct","spec": {"$block": "reference.kernel#/construct"}},"instantiate": {"kernel": "instantiate","spec": {"$block": "reference.kernel#/instantiate"}}}}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "construct",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:approval"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:constructapproval","dependencies": ["sha256:approval"]}
          }
        }
      },
      {
        "by": "self",
        "event": "e-approvalborn",
        "what": {
          "send": {
            "key": "spawn",
            "to": {"port": "instantiate"},
            "ref": "h-rinst",
            "attach": {},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "derived": {},
              "attach": {
                "port/left": {"handle": "h-aleft","spec": {"$block": "scenario.approval#/parts/opinion"}},
                "port/right": {"handle": "h-aright","spec": {"$block": "scenario.approval#/parts/opinion"}},
                "look": {"handle": "h-alook","spec": {"$block": "reference.kernel#/look"}},
                "notify": {"handle": "h-anotify","spec": {"$block": "reference.kernel#/notify"}},
                "correct": {"handle": "h-acorrect","spec": {"$block": "reference.kernel#/correct"}}
              },
              "accepted": true
            },
            "delivery": {"value": "sha256:spawnapproval","dependencies": ["sha256:approval"]}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "wait",
            "to": {"handle": "h-anotify"},
            "ref": "h-anotify",
            "attach": {"to": {"to": {"port": "back"},"ref": "h-rback"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {},
              "derived": {"to": {"handle": "h-adone","revoker": {"handle": "h-arev","spec": {"$block": "reference.kernel#/revoke"}}}},
              "accepted": true
            },
            "delivery": {"value": "sha256:empty","dependencies": []}
          }
        }
      },
      {
        "by": "self",
        "event": "e-left",
        "what": {
          "send": {
            "key": "left",
            "to": {"handle": "h-aleft"},
            "ref": "h-aleft",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:agree","dependencies": []}
          }
        }
      },
      {
        "by": "self",
        "event": "e-pauseapproval",
        "what": {
          "send": {
            "key": "pause",
            "to": {"handle": "h-acorrect"},
            "ref": "h-acorrect",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:pause","dependencies": []}
          }
        }
      },
      {
        "by": "self",
        "event": "e-right",
        "what": {
          "send": {
            "key": "right",
            "to": {"handle": "h-aright"},
            "ref": "h-aright",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:disagree","dependencies": []}
          }
        }
      },
      {
        "by": "self",
        "event": "e-resumeapproval",
        "what": {
          "send": {
            "key": "resume",
            "to": {"handle": "h-acorrect"},
            "ref": "h-acorrect",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:resume","dependencies": []}
          }
        }
      },
      {"by": "i-approval","event": "e-approvalend","what": {"arrival": {"port": "done","sender": "i-approval","via": "h-adone","attach": {},"delivery": {"value": "sha256:approvalout","dependencies": []}}}},
      {"by": "i-approval","event": "e-approvalend","what": {"invalid": ["h-aleft","h-aright"]}},
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["done"],"via": ["h-adone"]},"result": 8}}}
    ],
    "i-approval": [
      {"by": "i-root","event": "e-approvalborn","what": {"birth": {"definition": "sha256:approval","wire": {},"input": {"value": "sha256:null","dependencies": []}}}},
      {"by": "i-root","event": "e-left","what": {"arrival": {"port": "left","sender": "i-root","via": "h-aleft","attach": {},"delivery": {"value": "sha256:agree","dependencies": []}}}},
      {"by": "self","what": {"receive": {"key": "left","filter": {"ports": ["left"]},"result": 1}}},
      {"by": "i-root","event": "e-pauseapproval","what": {"correct": {"op": "pause"}}},
      {"by": "i-root","event": "e-right","what": {"arrival": {"port": "right","sender": "i-root","via": "h-aright","attach": {},"delivery": {"value": "sha256:disagree","dependencies": []}}}},
      {"by": "i-root","event": "e-resumeapproval","what": {"correct": {"op": "resume"}}},
      {"by": "self","what": {"receive": {"key": "right","filter": {"ports": ["right"]},"result": 4}}},
      {"by": "self","event": "e-approvalend","what": {"end": {"$block": "scenario.approval#/values/sha256:approvalout"}}}
    ]
  },
  "order": [{"first": ["i-approval",2],"then": ["i-root",5]}],
  "perturb": [{"crash": {"instance": "i-root","after": 6}}]
}
```

参考：README.md（格式、脚手架、扰动）；../README.md（制度机、场景）；../Instance.md（Control）；../Definition/Cognition.md（求值契约）。
