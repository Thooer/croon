# 根改代码并断线重连

场景：根让 W0 改仓库里的一处代码，收到结果写回磁盘；中途客户端断线。这一份不经子 Agent：W0 自己调一次 llm、一次 exec，以结束交回后一棵 快照 Bundle。写回磁盘是根在系统外的事，不进 Journal。

这里定义各场景共用的东西：root entry 与 W0、S 的 Definition，常用的 Port spec，root entry 的前缀 `root.born`、`root.start` 与收尾 `root.ended`，W0 的诞生与结束。root entry 提供 done 与 ask，诞生时在 self 里用 back、asks 把它们接到自己，ask 只在 PauseOnly 里用。

预期是 `scenario.reconnect`。

参考：README.md（格式、脚手架、扰动）；../README.md（场景）。
