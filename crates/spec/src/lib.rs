//! 从 docs/ 取出可检查、可取用的内容。
//!
//! `block` 取带名字的 fence block，`expand` 展开 `$block`，`generate` 为消费方生成规格表；`reference` 验文档与 Rust 模块注释的「参考：」行，
//! `glossary` 验名词表，`test_marker` 验测试标记，`tracked` 读配置，`limits` 与 `tokens` 量篇幅。
//! 参考：docs/Kernel/Engineering.md（接口）。

pub mod generate;
pub mod git;
pub mod limits;
pub mod tokens;
pub mod tracked;

mod block;
mod expand;
mod glossary;
mod reference;
mod test_marker;

pub use block::{Block, Error, blocks, blocks_in, docs, sources};
pub use expand::{ExpandError, expand};
pub use glossary::{GlossaryError, GlossaryName, check_glossary, glossary_names};
pub use reference::{Reference, ReferenceError, check_references, references};
pub use test_marker::{TestMarker, TestMarkerError, check_test_markers, test_markers};

pub use sha2;

use std::path::{Path, PathBuf};

/// 仓库根：`git rev-parse --show-toplevel`。工具从任意子目录运行都对得上。
pub fn repo_root() -> Result<PathBuf, String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("跑不了 git：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "不在 Git 仓库里：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_owned(),
    ))
}

/// 检查与取用的对象：docs/ 下全部 markdown，加上仓库根的 AGENTS.md。
pub(crate) fn documents(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    // 兼容三种布局：单目录 docs/、三仓库并排 croon/docs + croon-lang0/docs + croon-app/docs
    let legacy = root.join("docs");
    if legacy.is_dir() {
        files.extend(files_with_extension(&legacy, "md")?);
    }
    for sub in ["croon/docs", "croon-lang0/docs", "croon-app/docs"] {
        let d = root.join(sub);
        if d.is_dir() {
            files.extend(files_with_extension(&d, "md")?);
        }
    }
    let agents = root.join("AGENTS.md");
    if agents.is_file() {
        files.push(agents);
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// `dir` 下指定扩展名的文件，按路径排序；不跟随符号链接。
pub(crate) fn files_with_extension(dir: &Path, extension: &str) -> std::io::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&path, extension, out)?;
            } else if kind.is_file() && path.extension().is_some_and(|e| e == extension) {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    walk(dir, extension, &mut out)?;
    out.sort();
    Ok(out)
}
