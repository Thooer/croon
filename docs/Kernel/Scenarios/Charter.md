# 两个 Seat 的 Charter

场景：最小的 Charter，检验实例化一次事件里写下整张接线图。门占 Signature 所在的 Seat，它的一
个 Required port 接 Collective 的 Required port；审计者的一个 Required port 接门的 Provided
port。实例化之后两个成员都停在自己的取上：这一份只检验诞生，成员的结束与 Collective 的收尾见
../Instance.md 待定的终止闭环。

构造与实例化各查几条，见 ../Definition/README.md 的「检查」：构造查的是这份 Charter 自己的数
据，「不比来源宽」在那里做；实例化查「占座的与 Seat 的 Interface 相同」。所以这份预期里，门的
Required port 写的声明比来源窄一点也过得了。

预期是 `scenario.charter`。

参考：README.md（格式、脚手架）；../Definition/README.md（Charter、Seat、检查）；../Instance.md（成员的身份）。
