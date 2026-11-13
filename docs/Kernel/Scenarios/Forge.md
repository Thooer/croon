# 冒充子 Agent 的结束

场景：别人冒充 S 的结束交到 W0 的 done。冒充者是 root entry：实例化 W0 时它拿到了 W0 的 done，在登记之后交来一个伪造的结束状态。这项到达的 via 是 root entry 所经的那一份 h-w0done，sender 是 root entry；W0 按 via 取，取到的是 S 的那一项，伪造的到最后都没有被取。到达的序号因此后移，两边的取各自写出。

```json scenario.forge
{
  "values": {"sha256:forged": {"ok": true,"value": {"snapshot": "sha256:repo"},"dependencies": ["sha256:repo"]}},
  "journals": {
    "i-root": [
      {"$block": "scenario.reconnect#/parts/root.start"},
      {
        "by": "self",
        "event": "e-forge",
        "what": {
          "send": {
            "key": "forge",
            "to": {"handle": "h-w0done"},
            "ref": "h-w0done",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": null,"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:forged","dependencies": ["sha256:repo"]}
          }
        }
      },
      {"$block": "scenario.reconnect#/parts/root.ended"},
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["done"],"via": ["h-rdone"]},"result": 6}}}
    ],
    "i-w0": [
      {"$block": "scenario.delegate#/parts/w0.spawn"},
      {"$block": "scenario.delegate#/parts/w0.wait"},
      {"by": "i-root","event": "e-forge","what": {"arrival": {"port": "done","sender": "i-root","via": "h-w0done","attach": {},"delivery": {"value": "sha256:forged","dependencies": ["sha256:repo"]}}}},
      {"$block": "scenario.delegate#/parts/w0.arrival"},
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["done"],"via": ["h-sdone"]},"result": 4}}},
      {"$block": "scenario.delegate#/parts/w0.finish"}
    ],
    "i-s": {"$block": "scenario.delegate#/parts/s.journal"}
  }
}
```

参考：README.md（格式、脚手架）；Delegate.md；../README.md（场景）。
