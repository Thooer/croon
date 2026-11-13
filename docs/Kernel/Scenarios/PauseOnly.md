# 熔断者只能暂停

场景：登记者交回实例化出的 Instance 的 Correct，只想给暂停。不用 Charter：登记者是 root entry，它的 ask 的附带声明写明回话途径 reply 只收只能暂停的 Correct；熔断者 B 的 reply 与接到它的 me 写同一份 psreply。root entry 把对 W0 的完整 Correct 交到 B 的 reply，派生时接上这几节，B 手里的只能暂停：暂停在 W0 那里留下记录，解除暂停以 type 失败，因为违反的节里有 B 自己写的。W0 在途的生成随暂停丢弃。它也是 ../Reference.md 待定里「比相同」偏严的场景。

```json scenario.pauseonly
{
  "values": {
    "sha256:db": {
      "$bundle": {
        "language": "fake@1",
        "interface": {
          "description": "熔断者 B",
          "signature": {"in": {},"out": {}},
          "provides": {"reply": {"$block": "scenario.reconnect#/values/sha256:psreply"}},
          "requires": {"registrar": {"$block": "scenario.reconnect#/values/sha256:psask"},"me": {"$block": "scenario.reconnect#/values/sha256:psreply"}}
        },
        "body": "B"
      },
      "dependencies": []
    },
    "sha256:cb": {"definition": "sha256:db"},
    "sha256:spawnb": {"definition": "sha256:db","input": {"value": null,"dependencies": []},"seats": {},"self": {"me": "port/reply"}},
    "sha256:resume": {"op": "resume"}
  },
  "journals": {
    "i-root": [
      {"$block": "scenario.reconnect#/parts/root.born"},
      {
        "by": "self",
        "what": {
          "send": {
            "key": "cb",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:db"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:cb","dependencies": ["sha256:db"]}
          }
        }
      },
      {
        "by": "self",
        "event": "e-bborn",
        "what": {
          "send": {
            "key": "spawnb",
            "to": {"port": "instantiate"},
            "ref": "h-rinst",
            "attach": {"registrar": {"to": {"port": "asks"},"ref": "h-rasks"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {
                "port/reply": {"handle": "h-breply","spec": {"$block": "scenario.reconnect#/values/sha256:psreply"}},
                "look": {"handle": "h-blook","spec": {"$block": "reference.kernel#/look"}},
                "notify": {"handle": "h-bnotify","spec": {"$block": "reference.kernel#/notify"}},
                "correct": {"handle": "h-bcorrect","spec": {"$block": "reference.kernel#/correct"}}
              },
              "derived": {"registrar": {"handle": "h-bregistrar","revoker": {"handle": "h-rev11","spec": {"$block": "reference.kernel#/revoke"}}}},
              "accepted": true
            },
            "delivery": {"value": "sha256:spawnb","dependencies": ["sha256:db"]}
          }
        }
      },
      {
        "by": "i-b",
        "event": "e-ask",
        "what": {
          "arrival": {
            "port": "ask",
            "sender": "i-b",
            "via": "h-bregistrar",
            "attach": {"reply": {"handle": "h-rreply","spec": {"$block": "scenario.reconnect#/values/sha256:psreply"}}},
            "delivery": {"value": "sha256:null","dependencies": []}
          }
        }
      },
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["ask"]},"result": 6}}},
      {
        "by": "self",
        "event": "e-grant",
        "what": {
          "send": {
            "key": "grant",
            "to": {"handle": "h-rreply"},
            "ref": "h-rreply",
            "attach": {"correct": {"to": {"handle": "h-w0correct"},"ref": "h-w0correct"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {},
              "derived": {"correct": {"handle": "h-bpause","revoker": {"handle": "h-rev13","spec": {"$block": "reference.kernel#/revoke"}}}},
              "accepted": true
            },
            "delivery": {"value": "sha256:null","dependencies": []}
          }
        }
      },
      {"by": "i-b","event": "e-bend","what": {"invalid": ["h-breply","h-rreply"]}}
    ],
    "i-w0": [{"$block": "scenario.reconnect#/parts/w0.birth"},{"by": "i-b","event": "e-pausew0","what": {"correct": {"op": "pause"}}}],
    "i-b": [
      {"by": "i-root","event": "e-bborn","what": {"birth": {"definition": "sha256:db","wire": {"registrar": "h-bregistrar","me": "h-bme"},"input": {"value": "sha256:null","dependencies": []}}}},
      {
        "by": "self",
        "event": "e-ask",
        "what": {
          "send": {
            "key": "ask",
            "to": {"port": "registrar"},
            "ref": "h-bregistrar",
            "attach": {"reply": {"to": {"port": "me"},"ref": "h-bme"}},
            "result": {
              "outcome": {"ok": true,"value": null,"dependencies": []},
              "attach": {},
              "derived": {"reply": {"handle": "h-rreply","revoker": {"handle": "h-rev12","spec": {"$block": "reference.kernel#/revoke"}}}},
              "accepted": true
            },
            "delivery": {"value": "sha256:null","dependencies": []}
          }
        }
      },
      {
        "by": "i-root",
        "event": "e-grant",
        "what": {
          "arrival": {"port": "reply","sender": "i-root","via": "h-rreply","attach": {"correct": {"handle": "h-bpause","spec": {"$block": "reference.kernel#/correct"}}},"delivery": {"value": "sha256:null","dependencies": []}}
        }
      },
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["reply"],"via": ["h-rreply"]},"result": 2}}},
      {
        "by": "self",
        "event": "e-pausew0",
        "what": {
          "send": {
            "key": "pause",
            "to": {"handle": "h-bpause"},
            "ref": "h-bpause",
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
            "key": "resume",
            "to": {"handle": "h-bpause"},
            "ref": "h-bpause",
            "attach": {},
            "result": {"outcome": {"ok": false,"error": {"kind": "type","message": "","data": null},"dependencies": []},"attach": {},"derived": {},"accepted": false},
            "delivery": {"value": "sha256:resume","dependencies": []}
          }
        }
      },
      {"by": "self","event": "e-bend","what": {"end": {"ok": true,"value": null,"dependencies": []}}}
    ]
  },
  "pending": {"i-w0": [{"key": "llm1","send": {"to": {"port": "engine"},"delivery": {"value": {"$block": "scenario.reconnect#/values/sha256:prompt"},"dependencies": []},"attach": {}}}]}
}
```

参考：README.md（格式、脚手架）；Reconnect.md；../README.md（场景）；../Reference.md（待定）。
