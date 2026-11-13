//! 篇幅：AGENTS.md 的 `file.limits`，按 `docs/` 下每份 markdown、仓库根的 `AGENTS.md`
//! 与 `README.md` 的行数算。
//!
//! 小于 `min` 与超过 `danger` 不准提交，超过 `healthy` 只警告。取不到这份
//! `file.limits` 也不准提交。

use std::path::Path;

/// `file.limits` 的三个数。
pub struct Limits {
    pub min: usize,
    pub healthy: usize,
    pub danger: usize,
}

/// 一份 markdown 落在哪一档。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// 小于 `min`：不达标，应合并。
    Small,
    /// 在 `min` 与 `healthy` 之间。
    Healthy,
    /// 超过 `healthy`，还没到 `danger`：警告。
    OverHealthy,
    /// 超过 `danger`：应拆成文件夹。
    OverDanger,
}

pub struct Measure {
    pub path: String,
    pub lines: usize,
    pub verdict: Verdict,
}

/// 量篇幅的文件：`docs/` 下的 markdown（`docs/Draft.md` 除外），加仓库根的 `AGENTS.md` 与 `README.md`。
pub(crate) fn measured(path: &str) -> bool {
    if path == "AGENTS.md" || path == "README.md" {
        return true;
    }
    path.starts_with("docs/") && path.ends_with(".md") && path != "docs/Draft.md"
}

/// 量到的路径与行数，按路径排序。读的是 index，即将要提交的内容。
pub fn check(root: &Path) -> Result<Vec<Measure>, String> {
    let limits = limits(root)?;
    let listing = crate::git::run(root, &["ls-files", "-z"])?;
    let markdown: Vec<&str> = listing
        .split('\0')
        .filter(|path| !path.is_empty() && measured(path))
        .collect();

    let mut measures = Vec::with_capacity(markdown.len());
    for (path, bytes) in crate::git::blobs(root, ":", &markdown)? {
        let lines = String::from_utf8_lossy(&bytes).lines().count();
        let verdict = verdict(lines, &limits);
        measures.push(Measure {
            path,
            lines,
            verdict,
        });
    }
    Ok(measures)
}

/// 从 block `file.limits` 读三个数。
pub fn limits(root: &Path) -> Result<Limits, String> {
    let blocks = crate::docs(root).map_err(|e| e.to_string())?;
    let block = blocks
        .get("file.limits")
        .ok_or("AGENTS.md 里没有 `file.limits` 这个 block")?;
    let value: serde_json::Value = serde_json::from_str(&block.text)
        .map_err(|e| format!("{}:{} `file.limits`: {e}", block.file, block.line))?;
    let field = |name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as usize)
            .ok_or_else(|| format!("`file.limits` 里的 `{name}` 要是非负整数"))
    };
    let limits = Limits {
        min: field("min")?,
        healthy: field("healthy")?,
        danger: field("danger")?,
    };
    if !(limits.min <= limits.healthy && limits.healthy <= limits.danger) {
        return Err(format!(
            "`file.limits` 要 min ≤ healthy ≤ danger，现在是 {}、{}、{}",
            limits.min, limits.healthy, limits.danger
        ));
    }
    Ok(limits)
}

pub fn verdict(lines: usize, limits: &Limits) -> Verdict {
    if lines < limits.min {
        Verdict::Small
    } else if lines <= limits.healthy {
        Verdict::Healthy
    } else if lines <= limits.danger {
        Verdict::OverHealthy
    } else {
        Verdict::OverDanger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Limits {
        Limits {
            min: 15,
            healthy: 300,
            danger: 450,
        }
    }

    #[test]
    fn the_edges_belong_to_the_laxer_side() {
        assert_eq!(verdict(14, &sample()), Verdict::Small);
        assert_eq!(verdict(15, &sample()), Verdict::Healthy);
        assert_eq!(verdict(300, &sample()), Verdict::Healthy);
        assert_eq!(verdict(301, &sample()), Verdict::OverHealthy);
        assert_eq!(verdict(450, &sample()), Verdict::OverHealthy);
        assert_eq!(verdict(451, &sample()), Verdict::OverDanger);
    }

    #[test]
    fn the_measured_set_is_docs_markdown_plus_the_root_readme_and_agents() {
        assert!(measured("AGENTS.md"));
        assert!(measured("README.md"));
        assert!(measured("docs/Kernel/README.md"));
        assert!(!measured("crates/spec/README.md"));
        assert!(!measured("docs/Draft.md"));
        assert!(!measured("tracked.toml"));
    }

    #[test]
    fn the_repository_has_usable_limits() {
        let root = crate::repo_root().unwrap();
        let limits = limits(&root).unwrap();
        assert!(limits.min <= limits.healthy && limits.healthy <= limits.danger);
    }
}
