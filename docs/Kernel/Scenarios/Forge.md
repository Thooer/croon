# 冒充子 Agent 的结束

场景：别人冒充 S 的结束交到 W0 的 done。冒充者是 root entry：实例化 W0 时它拿到了 W0 的 done，在登记之后交来一个伪造的结束状态。这项到达的 via 是 root entry 所经的那一份 h-w0done，sender 是 root entry；W0 按 via 取，取到的是 S 的那一项，伪造的到最后都没有被取。到达的序号因此后移，两边的取各自写出。

预期是 `scenario.forge`。

参考：README.md（格式、脚手架）；Delegate.md；../README.md（场景）。
