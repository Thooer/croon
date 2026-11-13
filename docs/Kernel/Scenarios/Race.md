# 同时等几个 Agent

场景：W0 同时等两个 Agent，选最先结束的，其余不要了。W0 以同一份输入实例化 S 与 S2，各向 Notify 登记一次，to 都是 back，得到各自的 Handle；不按 via 限定地取，取到 S 的结束；暂停 S2，撤销登记给它的那一份，并入 S 交回的 快照 Bundle。S2 在途的生成随暂停丢弃，写在 `pending` 里。撤销的那一份由 Kernel 持有，撤销不造成别人的记录，见 ../Reference.md 的「撤销」。

```json scenario.race
{
  "values": {"sha256:pause": {"op": "pause"}},
  "journals": {
    "i-root": {"$block": "scenario.reconnect#/journals/i-root"},
    "i-w0": [
      {"$block": "scenario.delegate#/parts/w0.spawn"},
      {"$block": "scenario.delegate#/parts/w0.wait"},
      {
        "by": "self",
        "event": "e-s2born",
        "what": {
          "send": {
            "key": "spawn2",
            "to": {"port": "instantiate"},
            "ref": "h-w0inst",
            "attach": {"engine": {"to": {"port": "engine"},"ref": "h-w0engine"},"exec": {"to": {"port": "exec"},"ref": "h-w0exec"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {
                "look": {"handle": "h-s2look","spec": {"$block": "reference.kernel#/look"}},
                "notify": {"handle": "h-s2notify","spec": {"$block": "reference.kernel#/notify"}},
                "correct": {"handle": "h-s2correct","spec": {"$block": "reference.kernel#/correct"}}
              },
              "derived": {
                "engine": {"handle": "h-s2engine","revoker": {"handle": "h-rev8","spec": {"$block": "reference.kernel#/revoke"}}},
                "exec": {"handle": "h-s2exec","revoker": {"handle": "h-rev9","spec": {"$block": "reference.kernel#/revoke"}}}
              },
              "accepted": true
            },
            "delivery": {"value": "sha256:spawns","dependencies": ["sha256:ds","sha256:repo"]}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "wait2",
            "to": {"handle": "h-s2notify"},
            "ref": "h-s2notify",
            "attach": {"to": {"to": {"port": "back"},"ref": "h-w0back"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {},
              "derived": {"to": {"handle": "h-s2done","revoker": {"handle": "h-rev10","spec": {"$block": "reference.kernel#/revoke"}}}},
              "accepted": true
            },
            "delivery": {"value": "sha256:empty","dependencies": []}
          }
        }
      },
      {"$block": "scenario.delegate#/parts/w0.arrival"},
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["done"]},"result": 5}}},
      {
        "by": "self",
        "event": "e-pause2",
        "what": {
          "send": {
            "key": "pause2",
            "to": {"handle": "h-s2correct"},
            "ref": "h-s2correct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:pause","dependencies": []}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "drop2",
            "to": {"handle": "h-rev10"},
            "ref": "h-rev10",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:null","dependencies": []}
          }
        }
      },
      {"$block": "scenario.delegate#/parts/w0.finish"}
    ],
    "i-s": {"$block": "scenario.delegate#/parts/s.journal"},
    "i-s2": [
      {"by": "i-w0","event": "e-s2born","what": {"birth": {"definition": "sha256:ds","wire": {"engine": "h-s2engine","exec": "h-s2exec"},"input": {"value": "sha256:sin","dependencies": ["sha256:repo"]}}}},
      {"by": "i-w0","event": "e-pause2","what": {"correct": {"op": "pause"}}}
    ]
  },
  "pending": {"i-s2": [{"key": "llm1","send": {"to": {"port": "engine"},"delivery": {"value": {"$block": "scenario.reconnect#/values/sha256:prompt"},"dependencies": []},"attach": {}}}]}
}
```

参考：README.md（格式、脚手架）；Delegate.md；../README.md（场景）；../Reference.md（撤销）。
