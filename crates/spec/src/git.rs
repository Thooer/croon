//! 调 git，取 HEAD 或 index 里的内容。`full.md` 与「将要提交的内容」都从这里读。

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// 跑一条 git 命令，返回它的标准输出。
pub fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("git {}：{e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}：{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 把内容写成 blob，返回它的 ID。
pub fn hash_object(root: &Path, bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(["hash-object", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git hash-object：{e}"))?;
    child
        .stdin
        .take()
        .ok_or("git hash-object：没有 stdin")?
        .write_all(bytes)
        .map_err(|e| format!("git hash-object：{e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git hash-object：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git hash-object：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// 取出这些路径的内容，按给出的顺序。`prefix` 是 `HEAD:` 或 `:`，后者是 index。
pub fn blobs(root: &Path, prefix: &str, paths: &[&str]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git cat-file：{e}"))?;
    {
        let mut stdin = child.stdin.take().ok_or("git cat-file：没有 stdin")?;
        for path in paths {
            writeln!(stdin, "{prefix}{path}").map_err(|e| format!("git cat-file：{e}"))?;
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git cat-file：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git cat-file：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    // 每个对象先是 `<sha> blob <大小>` 一行，接着是这么多字节，再一个换行。
    let mut rest = &out.stdout[..];
    let mut blobs = Vec::with_capacity(paths.len());
    for path in paths {
        let end = rest
            .iter()
            .position(|b| *b == b'\n')
            .ok_or("git cat-file：输出被截断")?;
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("git cat-file：{prefix}{path} 取不到（{header}）"))?;
        let body = rest
            .get(end + 1..end + 1 + size)
            .ok_or("git cat-file：输出被截断")?;
        blobs.push((path.to_string(), body.to_vec()));
        rest = rest.get(end + 1 + size + 1..).unwrap_or(&[]);
    }
    Ok(blobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_index_and_head() {
        let root = crate::repo_root().unwrap();
        let index = run(&root, &["ls-files", "-z"]).unwrap();
        let paths: Vec<&str> = index.split('\0').filter(|p| !p.is_empty()).collect();
        assert!(paths.contains(&"AGENTS.md"));
        let blobs = blobs(&root, "HEAD:", &["AGENTS.md"]).unwrap();
        assert_eq!(blobs.len(), 1);
        assert!(String::from_utf8_lossy(&blobs[0].1).contains("file.limits"));
    }
}
