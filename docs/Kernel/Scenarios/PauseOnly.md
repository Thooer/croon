# 熔断者只能暂停

场景：登记者交回实例化出的 Instance 的 Correct，只想给暂停。不用 Charter：登记者是 root entry，它的 ask 的附带声明写明回话途径 reply 只收只能暂停的 Correct；熔断者 B 的 reply 与接到它的 me 写同一份 psreply。root entry 把对 W0 的完整 Correct 交到 B 的 reply，派生时接上这几节，B 手里的只能暂停：暂停在 W0 那里留下记录，解除暂停以 type 失败，因为违反的节里有 B 自己写的。W0 在途的生成随暂停丢弃。它也是 ../Reference.md 待定里「比相同」偏严的场景。

预期是 `scenario.pauseonly`。

参考：README.md（格式、脚手架）；Reconnect.md；../README.md（场景）；../Reference.md（待定）。
