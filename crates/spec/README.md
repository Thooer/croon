# spec

这个仓库的文档检查与快照生成，两个 bin：

- `check-spec`：提交前跑，查 AGENTS.md「检查」里能由代码验的规则。失败则不提交，只警告时放行。
- `merge-docs`：提交后跑，生成 `dist/full.md` 与 `dist/diff.md`，首行标着提交号，都不进 Git。

`dist/full.md` 收哪些文件写在仓库根的 `tracked.toml`：`include` 按其中的顺序收集，文件夹以 `/` 结尾、递归；`exclude` 命中的既不进 `full.md`，也不进 `diff.md` 的补丁。两个清单里的路径找不到时报出是哪一项。

模块：`block` 取带名字的 fence block，消费方通过 `sources` 列出构建输入，`generate` 共用展开与 OUT_DIR 表生成；`reference` 验文档与 Rust 模块注释的「参考：」行，`glossary` 验 `docs/README.md` 名词表里的名字唯一，`test_marker` 验正文里的 `[test:文件:用例]` 找得到，`limits` 按 `file.limits` 量 `docs/`、`AGENTS.md` 与 `README.md` 的行数，`tokens` 按 `context.limits` 与 `tracked.toml` 的 `[context]` 数正文的 token、并把实测值与超过健康值的文件写回 AGENTS.md 的 `context.usage`，`tracked` 读配置与选文件，`git` 取 HEAD 与 index 的内容。检查形状、名字、测试标记与参考用工作区；篇幅与 token 量暂存的内容，快照取 Git 对象。check-spec 通过后会写 Git index 中的 context.usage；只做只读验证时跑 `cargo test -p spec`，不运行提交钩子。

钩子在 `.githooks/`，装一次：`git config core.hooksPath .githooks`。
