# 无模型审批

根经一个 Entry 交出两类意见。审批者没有 Required port，也不收附带：分别从 left、right 取一项布尔意见，两项都同意才批准。根持有它的两个意见 Port、Look、Notify 与 Correct；向 Notify 登记之后交出第一项意见，等它取到，再暂停它。第二项意见在暂停期间到达，重启后仍保留，解除暂停后才被取。这里第二项是否决，所以决议为不批准。

这个场景检验单 Entry、固定审批规则、Control、通知与恢复，不把两类意见解释成两个已认证的参与者。Definition 的 fake@1 供共用脚手架使用；独立的审批求值器按同一规则计算结果，不从预期 Journal 生成程序。测试装配使用固定系统密钥，使同一次取得的两个 Port 失效时的 Handle 顺序也有固定预期。

预期是 `scenario.approval`。

参考：README.md（格式、脚手架、扰动）；../README.md（制度机、场景）；../Instance.md（Control）；../Definition/Cognition.md（求值契约）。
