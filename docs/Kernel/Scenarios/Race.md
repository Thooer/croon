# 同时等几个 Agent

场景：W0 同时等两个 Agent，选最先结束的，其余不要了。W0 以同一份输入实例化 S 与 S2，各向 Notify 登记一次，to 都是 back，得到各自的 Handle；不按 via 限定地取，取到 S 的结束；暂停 S2，撤销登记给它的那一份，并入 S 交回的 快照 Bundle。S2 在途的生成随暂停丢弃，写在 `pending` 里。撤销的那一份由 Kernel 持有，撤销不造成别人的记录，见 ../Reference.md 的「撤销」。

预期是 `scenario.race`。

参考：README.md（格式、脚手架）；Delegate.md；../README.md（场景）；../Reference.md（撤销）。
