# 形状

形状是 `definition.schema`。

构造时检查：

- 形状满足 definition.schema；Signature 与 Port 的 in、out 满足 value.schema；Provided port 没有 out。
- 完整对象满足 value.bundle，内容满足 definition.schema。检查只读这个闭包；其中引用的 Definition 必须在闭包里且已有构造事实。只缓存成功，缺依赖、正文拒绝不建立永久负面事实。
- Cognition：language 已登记并固定语义；这种 Language 检查 body 与显式依赖代码。宿主临时预算耗尽、装载故障按宿主故障，不记成正文拒绝。实际运行实现与资源可用性在 open 时检查。
- Charter：见 README.md 的「Charter」的「检查」。
- 错误带路径，并说出违反了哪条规则，LLM 能据此改正。

参考：README.md（Interface、Charter、完整身份与构造）；../Value.md（Schema、Bundle）；../Reference.md（能读的 ID）；../Language.md（守什么、实现）。
