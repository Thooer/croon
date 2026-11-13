//! 正文的 token 数：AGENTS.md 的 `context.limits` 与 `context.usage`。
//!
//! 数哪些文件由 `tracked.toml` 的 `[context]` 定。占模型窗口超过 `ratio` 不准提交。
//! 读的是 index，即将要提交的内容，不看工作区。通过之后，`stamp` 把实测值写回即将
//! 提交的 AGENTS.md 里的 `context.usage`，读 AGENTS.md 的模型就看得到现在占了多少。
//! 超过 `file.limits` 健康值的文档也列在这个 block 里，路径相对仓库根，一个文件一个行数。

use std::path::Path;

use tiktoken_rs::CoreBPE;

use crate::{git, tracked};

/// `context.limits` 的三个值。
pub struct Limits {
    pub window: u64,
    pub ratio: f64,
    pub tokenizer: String,
}

pub struct Measure {
    pub tokens: u64,
    pub files: usize,
    pub limits: Limits,
}

impl Measure {
    /// 占窗口的比例。
    pub fn share(&self) -> f64 {
        self.tokens as f64 / self.limits.window as f64
    }

    pub fn over(&self) -> bool {
        self.share() > self.limits.ratio
    }

    /// 一行说明。
    pub fn summary(&self) -> String {
        format!(
            "正文 {} 个文件，{} token（{}），占 {} 窗口的 {:.1}%，上限 {:.0}%",
            self.files,
            self.tokens,
            self.limits.tokenizer,
            self.limits.window,
            self.share() * 100.0,
            self.limits.ratio * 100.0
        )
    }
}

/// `stamp` 做了什么。
pub enum Stamp {
    /// 写进去的和已有的相同。
    Unchanged,
    /// 写进了 index；`synced` 是工作区的 AGENTS.md 也跟着改了。工作区有没提交的改动时
    /// 不动它，免得盖掉。
    Written { synced: bool },
}

/// 数正文的 token。
pub fn measure(root: &Path) -> Result<Measure, String> {
    let limits = limits(root)?;
    let bpe = bpe(&limits.tokenizer)?;
    let config = tracked::config(root)?;

    let listing = git::run(root, &["ls-files", "-z"])?;
    let paths: Vec<&str> = listing.split('\0').filter(|p| !p.is_empty()).collect();
    let selected = tracked::select(
        "context.",
        &config.context.include,
        &config.context.exclude,
        &paths,
    )?;

    let mut tokens = 0u64;
    let mut files = 0;
    for (_, bytes) in git::blobs(root, ":", &selected)? {
        if bytes.contains(&0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        tokens += bpe.encode_ordinary(&neutral(&text)).len() as u64;
        files += 1;
    }
    Ok(Measure {
        tokens,
        files,
        limits,
    })
}

const USAGE: &str = "```json context.usage\n";

/// `context.usage` 的内容换成空对象再数：写进去的数不影响它自己，同一份文档数两遍得到
/// 同一个数。代价是实测值不含这个 block 自己，几十个 token。
fn neutral(text: &str) -> String {
    with_usage(text, "{}").unwrap_or_else(|_| text.to_owned())
}

/// 把 `context.usage` 的内容换成 `body`；找不到这个 block 就报错。
fn with_usage(text: &str, body: &str) -> Result<String, String> {
    let start = text
        .find(USAGE)
        .ok_or("AGENTS.md 里没有 `context.usage` 这个 block")?
        + USAGE.len();
    let end = text[start..]
        .find("\n```")
        .ok_or("`context.usage` 这个 block 没有收尾")?
        + start;
    Ok(format!("{}{body}{}", &text[..start], &text[end..]))
}

/// 要写进 `context.usage` 的内容。token 取整到百位，小改动不会让每次提交都带一个新数。
/// `over_healthy` 是超过健康值的文档与行数，按路径排序；没有就写空对象。
fn usage_body(measure: &Measure, over_healthy: &[(String, usize)]) -> String {
    let rounded = (measure.tokens + 50) / 100 * 100;
    let mut files: Vec<&(String, usize)> = over_healthy.iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let field = if files.is_empty() {
        "{}".to_owned()
    } else {
        let lines: Vec<String> = files
            .iter()
            .map(|(path, count)| {
                let path = serde_json::to_string(path).unwrap_or_else(|_| format!("{path:?}"));
                format!("    {path}: {count}")
            })
            .collect();
        format!("{{\n{}\n  }}", lines.join(",\n"))
    };
    format!(
        "{{\n  \"tokens\": {rounded},\n  \"percent\": {:.1},\n  \"over_healthy\": {field}\n}}",
        measure.share() * 100.0
    )
}

/// 把实测值写进 index 里的 AGENTS.md。只动 index 里的那份：提交里带上，部分暂存也不
/// 会把别的改动一起带进去。
pub fn stamp(
    root: &Path,
    measure: &Measure,
    over_healthy: &[(String, usize)],
) -> Result<Stamp, String> {
    let blobs = git::blobs(root, ":", &["AGENTS.md"])?;
    let old =
        String::from_utf8(blobs[0].1.clone()).map_err(|e| format!("AGENTS.md 不是 UTF-8：{e}"))?;
    let new = with_usage(&old, &usage_body(measure, over_healthy))?;
    if new == old {
        return Ok(Stamp::Unchanged);
    }

    let listing = git::run(root, &["ls-files", "-s", "--", "AGENTS.md"])?;
    let mode = listing
        .split_whitespace()
        .next()
        .ok_or("git ls-files：AGENTS.md 不在 index 里")?;
    let sha = git::hash_object(root, new.as_bytes())?;
    git::run(
        root,
        &[
            "update-index",
            "--cacheinfo",
            &format!("{mode},{sha},AGENTS.md"),
        ],
    )?;

    let path = root.join("AGENTS.md");
    let synced = std::fs::read_to_string(&path).is_ok_and(|text| text == old);
    if synced {
        std::fs::write(&path, &new).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(Stamp::Written { synced })
}

fn bpe(name: &str) -> Result<&'static CoreBPE, String> {
    match name {
        "o200k_base" => Ok(tiktoken_rs::o200k_base_singleton()),
        "cl100k_base" => Ok(tiktoken_rs::cl100k_base_singleton()),
        other => Err(format!(
            "`context.limits` 的 tokenizer 只认 o200k_base 与 cl100k_base，现在是 `{other}`"
        )),
    }
}

/// 从 block `context.limits` 读三个值。
pub fn limits(root: &Path) -> Result<Limits, String> {
    let blocks = crate::docs(root).map_err(|e| e.to_string())?;
    let block = blocks
        .get("context.limits")
        .ok_or("AGENTS.md 里没有 `context.limits` 这个 block")?;
    parse(&block.text).map_err(|e| format!("{}:{} `context.limits`: {e}", block.file, block.line))
}

fn parse(text: &str) -> Result<Limits, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let window = value
        .get("window")
        .and_then(serde_json::Value::as_u64)
        .filter(|n| *n > 0)
        .ok_or("`window` 要是正整数")?;
    let ratio = value
        .get("ratio")
        .and_then(serde_json::Value::as_f64)
        .filter(|r| *r > 0.0 && *r <= 1.0)
        .ok_or("`ratio` 要在 0 到 1 之间")?;
    let tokenizer = value
        .get("tokenizer")
        .and_then(serde_json::Value::as_str)
        .ok_or("`tokenizer` 要是字符串")?
        .to_owned();
    Ok(Limits {
        window,
        ratio,
        tokenizer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(tokens: u64) -> Measure {
        Measure {
            tokens,
            files: 1,
            limits: Limits {
                window: 1000,
                ratio: 0.2,
                tokenizer: "o200k_base".into(),
            },
        }
    }

    #[test]
    fn the_share_is_checked_against_the_ratio() {
        assert!(!measure(200).over());
        assert!(measure(201).over());
    }

    #[test]
    fn the_stamped_number_does_not_change_the_count() {
        let doc = |body: &str| format!("前\n\n{USAGE}{body}\n```\n\n后\n");
        let a = doc("{\n  \"tokens\": 100,\n  \"percent\": 0.0\n}");
        let b = doc("{\n  \"tokens\": 123456,\n  \"percent\": 12.3\n}");
        assert_eq!(neutral(&a), neutral(&b));
        assert_eq!(with_usage(&a, "{}").unwrap(), neutral(&a));
    }

    #[test]
    fn stamping_replaces_only_the_block() {
        let text = format!("前\n\n{USAGE}{{}}\n```\n\n后\n");
        let body = usage_body(&measure(12_340), &[]);
        let stamped = with_usage(&text, &body).unwrap();
        assert!(stamped.starts_with("前\n\n"));
        assert!(stamped.ends_with("\n```\n\n后\n"));
        assert!(stamped.contains("\"tokens\": 12300"), "{stamped}");
        assert!(with_usage("没有 block", &body).is_err());
    }

    #[test]
    fn tokens_round_to_the_nearest_hundred() {
        assert!(usage_body(&measure(149), &[]).contains("\"tokens\": 100"));
        assert!(usage_body(&measure(150), &[]).contains("\"tokens\": 200"));
    }

    #[test]
    fn over_healthy_files_are_listed_by_path_with_their_line_counts() {
        let files = vec![("docs/b.md".to_owned(), 301), ("docs/a.md".to_owned(), 260)];
        let body = usage_body(&measure(12_340), &files);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["tokens"], 12_300);
        assert_eq!(value["over_healthy"]["docs/a.md"], 260);
        assert_eq!(value["over_healthy"]["docs/b.md"], 301);
        let a = body.find("docs/a.md").unwrap();
        let b = body.find("docs/b.md").unwrap();
        assert!(a < b, "按路径排序：{body}");
    }

    #[test]
    fn no_over_healthy_files_is_an_empty_object() {
        let body = usage_body(&measure(100), &[]);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(value["over_healthy"].as_object().unwrap().is_empty());
    }

    #[test]
    fn bad_limits_are_refused() {
        assert!(parse(r#"{"window":0,"ratio":0.2,"tokenizer":"o200k_base"}"#).is_err());
        assert!(parse(r#"{"window":1000,"ratio":1.5,"tokenizer":"o200k_base"}"#).is_err());
        assert!(parse(r#"{"window":1000,"ratio":0.2}"#).is_err());
        assert!(bpe("unknown").is_err());
    }

    #[test]
    fn the_repository_has_usable_limits() {
        let root = crate::repo_root().unwrap();
        let limits = limits(&root).unwrap();
        assert!(bpe(&limits.tokenizer).is_ok());
    }
}
