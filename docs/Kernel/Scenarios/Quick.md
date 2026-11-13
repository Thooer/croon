# 子 Agent 很快

场景：S 很快，W0 向 Notify 登记时它已经结束。与 Delegate 的差别只在三处：S 的结束没有造成任何记录，所以不带事件；到达在登记的同一个事务里交来，by 是 self，带登记那一步的事件；W0 的取不用改。S 先结束推不出，写在 `order` 里。

```json scenario.quick
{
  "journals": {
    "i-root": {"$block": "scenario.reconnect#/journals/i-root"},
    "i-w0": [
      {"$block": "scenario.delegate#/parts/w0.spawn"},
      {"$block": "scenario.delegate#/parts/w0.wait","event": "e-wait"},
      {"by": "self","event": "e-wait","what": {"arrival": {"port": "done","sender": "i-s","via": "h-sdone","attach": {},"delivery": {"value": "sha256:out","dependencies": ["sha256:repo2"]}}}},
      {"by": "self","what": {"receive": {"key": "take","filter": {"ports": ["done"],"via": ["h-sdone"]},"result": 3}}},
      {"$block": "scenario.delegate#/parts/w0.finish"}
    ],
    "i-s": [{"$block": "scenario.delegate#/parts/s.work"},{"by": "self","what": {"end": {"$block": "scenario.reconnect#/values/sha256:out"}}}]
  },
  "order": [{"first": ["i-s",3],"then": ["i-w0",2]}]
}
```

参考：README.md（格式、脚手架）；Delegate.md；../README.md（场景）。
