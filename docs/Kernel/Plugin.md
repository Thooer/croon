# Plugin

Plugin 是 Kernel 的两个插口——Language 与 Port 提供者——的宿主代码形态：由根装，装不装由根决定；它提供的无来源 Reference 只能从根接入；代码谁写都行。Language 的实现是它的一种，见 Language 的「插口」。

Port 的实现与 Reference 的所指分开：根诞生的 binding 固定来源与规格；宿主按来源 Handle 装配 Port，实现可换，但已接受的执行保留原对象与通道。宿主没装实现或收取故障不写永久 use 失败：内部局部停下，外部 Entry 同键重报可继续收取；已有 Journal 与索引照常重建。 仅 Kernel construct 成功建立构造事实，同形 Plugin 的结果不自动建立。

## 判据

按 README.md 的「放置」决定是否需要 Plugin。通道与存储是 Kernel 的实现，后端可换，不是 Plugin。

## 可信

宿主代码的可信按保证分开：Language 的实现关系到每个 Instance 一个隔离单元、两个 Atom 之间的有界由宿主一侧兜住，见 Language 的「插口」；Port 的提供者关系到凭据，以及效果是否只经通道；容器运行时关系到效果框在工作区；存储关系到全部保证。

可信，指某条保证依赖这段代码，与谁写的无关。Definition 不需要信任：Kernel 检查它，写错了只会失败。Plugin 是宿主代码，它的可信由根承担。

## 待定

- Plugin 的边界，待验证：插口只有 Port 与 Language 两个；Plugin 提供的 Port，凭据留在 Plugin 里。Kernel 提供的原始 Port 越多（如通用的出站 Port），要做成 Plugin 的越少，这一点与 Reference 的待定「联网」一起定。具体的候选是 MCP 客户端：外部的 tools 会变，Port spec 在构造时固定，所以要选每个 tool 快照成一份 Port spec，还是一个通用的调用、tool 名作为 in 里的字段并以 enum 收窄；本地 stdio 的 MCP server 是长驻的进程，进程不跨 Atom，所以进不了 exec，只能作为外部服务接入（推断）。场景：接外部服务。

参考：README.md（放置）；Language.md（插口、守什么）；Definition/README.md（Interface、Port）；Reference.md（通道、待定）；Port.md（Kernel 的 Port）。
