# 根改代码并断线重连

场景：根让 W0 改仓库里的一处代码，收到结果写回磁盘；中途客户端断线。这一份不经子 Agent：W0 自己调一次 llm、一次 exec，以结束交回后一棵 快照 Bundle。写回磁盘是根在系统外的事，不进 Journal。

这里定义各场景共用的东西：root entry 与 W0、S 的 Definition，常用的 Port spec，root entry 的前缀 `root.born`、`root.start` 与收尾 `root.ended`，W0 的诞生与结束。root entry 提供 done 与 ask，诞生时在 self 里用 back、asks 把它们接到自己，ask 只在 PauseOnly 里用。

```json scenario.reconnect
{
  "values": {
    "sha256:droot": {
      "$bundle": {
        "evaluator": "external",
        "interface": {
          "description": "root entry",
          "signature": {"in": {},"out": {}},
          "provides": {"done": {"$block": "scenario.reconnect#/values/sha256:psdone"},"ask": {"$block": "scenario.reconnect#/values/sha256:psask"}},
          "requires": {
            "construct": {"$block": "reference.kernel#/construct"},
            "instantiate": {"$block": "reference.kernel#/instantiate"},
            "engine": {"$block": "scenario.engine"},
            "exec": {"$block": "scenario.exec"},
            "back": {"$block": "scenario.reconnect#/values/sha256:psdone"},
            "asks": {"$block": "scenario.reconnect#/values/sha256:psask"}
          }
        }
      },
      "dependencies": []
    },
    "sha256:dw0": {
      "$bundle": {
        "language": "fake@1",
        "interface": {
          "description": "W0：改代码，或交办给 worker",
          "signature": {"in": {},"out": {}},
          "provides": {"done": {"$block": "scenario.reconnect#/values/sha256:psdone"}},
          "requires": {"engine": {"$block": "scenario.engine"},"exec": {"$block": "scenario.exec"},"instantiate": {"$block": "reference.kernel#/instantiate"},"back": {"$block": "scenario.reconnect#/values/sha256:psdone"}}
        },
        "body": "W0"
      },
      "dependencies": []
    },
    "sha256:ds": {
      "$bundle": {
        "language": "fake@1",
        "interface": {"description": "S：改一处代码","signature": {"in": {},"out": {}},"provides": {},"requires": {"engine": {"$block": "scenario.engine"},"exec": {"$block": "scenario.exec"}}},
        "body": "S"
      },
      "dependencies": []
    },
    "sha256:psdone": {"description": "收结束状态","in": {"$block": "definition.outcome"},"attach": {"additionalProperties": false}},
    "sha256:psask": {"description": "要一份只能暂停的 Correct","in": {},"attach": {"properties": {"reply": {"$block": "scenario.reconnect#/values/sha256:psreply"}},"required": ["reply"],"additionalProperties": false}},
    "sha256:psreply": {"description": "收一份只能暂停的 Correct","in": {"type": "null"},"attach": {"properties": {"correct": {"$block": "scenario.reconnect#/values/sha256:pspause"}},"required": ["correct"],"additionalProperties": false}},
    "sha256:pspause": {"$block": "reference.kernel#/correct","description": "只能暂停的 Correct","in": {"$block": "reference.kernel#/correct/in/$defs/pause"},"attach": {"additionalProperties": false}},
    "sha256:pslook": {"$block": "reference.kernel#/look"},
    "sha256:psnotify": {"$block": "reference.kernel#/notify"},
    "sha256:pscorrect": {"$block": "reference.kernel#/correct"},
    "sha256:psrevoke": {"$block": "reference.kernel#/revoke"},
    "sha256:null": null,
    "sha256:empty": {},
    "sha256:repo": {"$bundle": {"marker.txt": {"file": "sha256:beforefile","x": false}},"dependencies": ["sha256:beforefile"]},
    "sha256:cw0": {"definition": "sha256:dw0"},
    "sha256:cs": {"definition": "sha256:ds"},
    "sha256:spawn": {"definition": "sha256:dw0","input": {"value": {"task": "fix","worker": "sha256:ds","snapshot": "sha256:repo"},"dependencies": ["sha256:ds","sha256:repo"]},"seats": {},"self": {"back": "port/done"}},
    "sha256:w0in": {"$block": "scenario.reconnect#/values/sha256:spawn/input/value"},
    "sha256:prompt": {"system": "","messages": [{"role": "user","text": "fix"}],"tools": []},
    "sha256:edit": {"snapshot": "sha256:repo","argv": ["edit"],"mounts": {},"stdin": "","timeout": 60},
    "sha256:out": {"ok": true,"value": {"snapshot": "sha256:repo2"},"dependencies": ["sha256:repo2"]},
    "sha256:repo2": {"$bundle": {"marker.txt": {"file": "sha256:afterfile","x": false}},"dependencies": ["sha256:afterfile"]},
    "sha256:beforefile": {"$bytes": "before\n"},
    "sha256:afterfile": {"$bytes": "after\n"}
  },
  "parts": {
    "llm": {"outcome": {"ok": true,"value": {"text": "edit","reasoning": null,"calls": [],"finish": "stop","usage": {"input": 1,"output": 1}},"dependencies": []},"attach": {},"derived": {},"accepted": true},
    "edit": {
      "outcome": {"ok": true,"value": {"exit": 0,"stdout": "","stderr": "","timed_out": false,"truncated": false,"snapshot": "sha256:repo2"},"dependencies": ["sha256:repo2"]},
      "attach": {},
      "derived": {},
      "accepted": true
    },
    "root.born": [
      {
        "by": "outside",
        "what": {
          "birth": {
            "definition": "sha256:droot",
            "wire": {"construct": "h-rconstruct","instantiate": "h-rinst","engine": "h-rengine","exec": "h-rexec","back": "h-rback","asks": "h-rasks"},
            "self": {"back": "port/done","asks": "port/ask"},
            "input": {"value": "sha256:null","dependencies": ["sha256:dw0","sha256:ds","sha256:db","sha256:repo"]},
            "binding": {
              "sources": {
                "construct": {"kernel": "construct","spec": {"$block": "reference.kernel#/construct"}},
                "instantiate": {"kernel": "instantiate","spec": {"$block": "reference.kernel#/instantiate"}},
                "engine": {"host": "fake-engine","spec": {"$block": "scenario.engine"}},
                "exec": {"host": "fake-exec","spec": {"$block": "scenario.exec"}}
              }
            }
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "cs",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:ds"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:cs","dependencies": ["sha256:ds"]}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "cw0",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:dw0"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:cw0","dependencies": ["sha256:dw0"]}
          }
        }
      },
      {
        "by": "self",
        "event": "e-w0born",
        "what": {
          "send": {
            "key": "spawn",
            "to": {"port": "instantiate"},
            "ref": "h-rinst",
            "attach": {"engine": {"to": {"port": "engine"},"ref": "h-rengine"},"exec": {"to": {"port": "exec"},"ref": "h-rexec"},"instantiate": {"to": {"port": "instantiate"},"ref": "h-rinst"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {
                "port/done": {"handle": "h-w0done","spec": {"$block": "scenario.reconnect#/values/sha256:psdone"}},
                "look": {"handle": "h-w0look","spec": {"$block": "reference.kernel#/look"}},
                "notify": {"handle": "h-w0notify","spec": {"$block": "reference.kernel#/notify"}},
                "correct": {"handle": "h-w0correct","spec": {"$block": "reference.kernel#/correct"}}
              },
              "derived": {
                "engine": {"handle": "h-w0engine","revoker": {"handle": "h-rev1","spec": {"$block": "reference.kernel#/revoke"}}},
                "exec": {"handle": "h-w0exec","revoker": {"handle": "h-rev2","spec": {"$block": "reference.kernel#/revoke"}}},
                "instantiate": {"handle": "h-w0inst","revoker": {"handle": "h-rev3","spec": {"$block": "reference.kernel#/revoke"}}}
              },
              "accepted": true
            },
            "delivery": {"value": "sha256:spawn","dependencies": ["sha256:dw0","sha256:ds","sha256:repo"]}
          }
        }
      }
    ],
    "root.start": [
      {"$block": "scenario.reconnect#/parts/root.born"},
      {
        "by": "self",
        "what": {
          "send": {
            "key": "wait",
            "to": {"handle": "h-w0notify"},
            "ref": "h-w0notify",
            "attach": {"to": {"to": {"port": "back"},"ref": "h-rback"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {},
              "derived": {"to": {"handle": "h-rdone","revoker": {"handle": "h-rev4","spec": {"$block": "reference.kernel#/revoke"}}}},
              "accepted": true
            },
            "delivery": {"value": "sha256:empty","dependencies": []}
          }
        }
      }
    ],
    "root.ended": [
      {"by": "i-w0","event": "e-w0end","what": {"arrival": {"port": "done","sender": "i-w0","via": "h-rdone","attach": {},"delivery": {"value": "sha256:out","dependencies": ["sha256:repo2"]}}}},
      {"by": "i-w0","event": "e-w0end","what": {"invalid": ["h-w0done"]}}
    ],
    "w0.birth": {
      "by": "i-root",
      "event": "e-w0born",
      "what": {
        "birth": {"definition": "sha256:dw0","wire": {"engine": "h-w0engine","exec": "h-w0exec","instantiate": "h-w0inst","back": "h-w0back"},"input": {"value": "sha256:w0in","dependencies": ["sha256:ds","sha256:repo"]}}
      }
    },
    "w0.end": {"by": "self","event": "e-w0end","what": {"end": {"$block": "scenario.reconnect#/values/sha256:out"}}}
  },
  "journals": {
    "i-root": [
      {"$block": "scenario.reconnect#/parts/root.start"},
      {"$block": "scenario.reconnect#/parts/root.ended"},
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["done"],"via": ["h-rdone"]},"result": 5}}}
    ],
    "i-w0": [
      {"$block": "scenario.reconnect#/parts/w0.birth"},
      {
        "by": "self",
        "what": {"send": {"key": "llm1","to": {"port": "engine"},"ref": "h-w0engine","attach": {},"result": {"$block": "scenario.reconnect#/parts/llm"},"delivery": {"value": "sha256:prompt","dependencies": []}}}
      },
      {
        "by": "self",
        "what": {"send": {"key": "edit","to": {"port": "exec"},"ref": "h-w0exec","attach": {},"result": {"$block": "scenario.reconnect#/parts/edit"},"delivery": {"value": "sha256:edit","dependencies": ["sha256:repo"]}}}
      },
      {"$block": "scenario.reconnect#/parts/w0.end"}
    ]
  },
  "perturb": [{"reconnect": {"after": 3,"resend": ["spawn"],"stale": ["wait"]}},{"crash": {"instance": "i-w0","key": "edit"}}]
}
```

参考：README.md（格式、脚手架、扰动）；../README.md（场景）。
