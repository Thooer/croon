# W0 交办给子 Agent

场景：W0 把一处修改交办给子 Agent S，等它结束，审过交回的 快照 Bundle 再并入。root entry 一侧与 Reconnect 相同，整本引用。W0 以自己当前的 快照 Bundle 为 S 的初始 快照 Bundle，接上 engine 与 exec；向 Notify 登记时附带接到自己 done 上的 back，按 via 取；审阅与并入合成一次 exec，挂上 S 交回的 快照 Bundle。

这里定义 W0 交办的前缀 `w0.spawn`、登记 `w0.wait`、到达 `w0.arrival` 与收尾 `w0.finish`，S 的 `s.work` 与 `s.journal`，供 Quick、Forge、Race 引用。

预期是 `scenario.delegate`。

参考：README.md（格式、脚手架）；Reconnect.md；../README.md（场景）。
