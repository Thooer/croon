# 会话

Kernel 与 Language 之间是一次会话。开始时给出诞生和已有 Atom 结果；之后 Language 报新 Atom，Kernel 送回结果，直到 done。恢复时重新打开会话，结果按键匹配；输出不得依赖结果送回的先后。具体保证见 Cognition.md。

诞生固定完整 Definition Bundle、输入交付、Seat 与名册；不含默认业务状态。名册只列接到该成员 Provided port 的 Seat。Definition 内容随诞生给出，检查和代码装载只读它的固定闭包；运行中的业务读取按程序实际消费的结果推进资格。

宿主调用的错误返回独立于 JSON；open、resume、读取或宿主预算故障都停下求值，不伪造 body 的结果。某次结果已经提交、尚未送回时保留原值，恢复不重新执行。

进来的 Reference 携带 Handle 与完整 Port spec。规格本身是数据，交给 Language 判断本地类型，不从规格猜提供者。Handle 只按该 Instance 的 c-list 解析。派生链的真实规格与位置由 Kernel 保存。

会话的形状是 `definition.eval`，结束与交出的结果是 `definition.outcome`。

outcome.dependencies 是实际交付的根，成功、失败都能有产物；它不受 accepted 的真假推导。accepted 只解释请求是否接下，不能授予结果里未出现的根。宿主输出违反 out 时，业务值改为 use 失败，已成立的产物仍在失败结果的 dependencies 中交付；候选诞生、派生和到达不提交。

案例：A 请求执行 R0，命令失败但产生 R1，accepted 为 true，失败结果交付 R1；A 请求一个没有读取资格的根，accepted 为 false、结果依赖为空。重建不用从失败 kind 猜接受事实。难改的点是结果的资格与请求位置，权威见 Reference。

参考：Cognition.md（求值契约、失败）；../Value.md（Bundle、交付）；../Reference.md（能读的 ID）。
