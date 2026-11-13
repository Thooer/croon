//! 生成两份喂给模型的项目快照，都对应 HEAD，都不进 Git：
//!
//! - `dist/full.md`：收哪些文件由仓库根的 `tracked.toml` 定，一个文件一段，围栏按
//!   内容里最长的反引号串选。
//! - `dist/diff.md`：HEAD 的完整补丁，连同提交信息；`tracked.toml` 排除掉的不进。
//!
//! 内容都从 git 对象里取，不看工作区，所以工作区有未提交改动也不影响。二进制文件跳过。

use std::path::Path;
use std::process::ExitCode;

use spec::git;
use spec::tracked;

fn main() -> ExitCode {
    let result = spec::repo_root().and_then(|root| run(&root));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(root: &Path) -> Result<(), String> {
    let config = tracked::config(root)?;
    let hash = git::run(root, &["rev-parse", "--short", "HEAD"])?;
    let dist = root.join("dist");
    std::fs::create_dir_all(&dist).map_err(|e| format!("{}: {e}", dist.display()))?;

    // full.md
    let listing = git::run(root, &["ls-tree", "-r", "-z", "--name-only", "HEAD"])?;
    let paths: Vec<&str> = listing.split('\0').filter(|p| !p.is_empty()).collect();
    let selected = tracked::select("", &config.include, &config.exclude, &paths)?;
    let mut parts = vec![format!("<!-- HEAD {} -->", hash.trim())];
    let (mut merged, mut skipped) = (0, 0);
    for (path, bytes) in git::blobs(root, "HEAD:", &selected)? {
        if bytes.contains(&0) {
            skipped += 1;
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let fence = fence_for(&text);
        parts.push(format!(
            "# {path}\n\n{fence}\n{}\n{fence}\n",
            text.trim_end()
        ));
        merged += 1;
    }
    let full = parts.join("\n");
    write(&dist.join("full.md"), &full)?;

    // diff.md：排除按文件段落去掉，不交给 pathspec。文件从一个路径挪到被排除的
    // 路径时，pathspec 会让 git 认不出这是改名，于是把旧路径当删除，整份内容进补丁。
    let diff = git::run(root, &["show", "--patch", "--no-color", "HEAD"])?;
    let diff = without_excluded(&diff, &config.exclude);
    let fence = fence_for(&diff);
    let file = format!(
        "<!-- HEAD {} -->\n\n{fence}\n{}\n{fence}\n",
        hash.trim(),
        diff.trim_end()
    );
    write(&dist.join("diff.md"), &file)?;

    println!(
        "full.md：{merged} 个文件，跳过 {skipped} 个二进制文件，{} 字节",
        full.len()
    );
    println!("diff.md：{} 行，{} 字节", diff.lines().count(), file.len());
    println!("输出：{}", dist.display());
    Ok(())
}

/// 去掉路径命中 `exclude` 的文件段落；提交信息那一段不动。
/// 每段以 `diff --git a/<路径> b/<路径>` 开头，两边任一命中就去掉。
fn without_excluded(diff: &str, exclude: &[String]) -> String {
    let mut out = String::with_capacity(diff.len());
    let mut keep = true;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            keep = !paths(rest)
                .iter()
                .any(|path| exclude.iter().any(|entry| tracked::matched(path, entry)));
        }
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// 从 `a/<路径> b/<路径>` 里取出两个路径。
fn paths(head: &str) -> [&str; 2] {
    let mut found = ["", ""];
    for (i, part) in head.split_whitespace().take(2).enumerate() {
        found[i] = part
            .strip_prefix("a/")
            .or_else(|| part.strip_prefix("b/"))
            .unwrap_or(part);
    }
    found
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// 按内容里最长的反引号串选围栏，围住的文本里不会出现结束围栏。
fn fence_for(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_files_drop_their_whole_section() {
        let diff = "commit 1\n\ndiff --git a/docs/a.md b/docs/a.md\n--- a\n+++ b\n+保留\n\
                    diff --git a/docs/Old.md b/Draft.md\nrename from docs/Old.md\nrename to Draft.md\n\
                    diff --git a/Draft.md b/Draft.md\n+-私密\n";
        let kept = without_excluded(diff, &["Draft.md".to_string()]);
        assert!(kept.contains("+保留"));
        assert!(!kept.contains("私密"));
        assert!(!kept.contains("rename to Draft.md"));
    }

    #[test]
    fn fence_is_longer_than_any_run_inside() {
        assert_eq!(fence_for("没有反引号"), "```");
        assert_eq!(fence_for("`` 两个"), "```");
        assert_eq!(fence_for("```\n代码块\n```"), "````");
    }
}
