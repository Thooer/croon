//! 取出 docs/ 下带名字的 fence block。
//! 参考：docs/Kernel/Engineering.md（接口）；docs/Kernel/Value.md（内容寻址）。
//!
//! 名字是语言标签后的一个词，如 `json value.tree`、`mermaid instance.status`。
//! 这里只管取出与唯一性；每种语言怎样解读由调用方负责。

use std::collections::BTreeMap;
use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

/// 一个带名字的 block。`line` 是围栏在源文件里的行号，报错指回这里。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    pub lang: String,
    pub text: String,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{file}:{line}: block `{name}` 被定义了两次")]
    Duplicate {
        name: String,
        file: String,
        line: usize,
    },
    #[error("{file}: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
}

impl Block {
    /// 规范文本的 SHA-256，六十四位十六进制。
    pub fn sha256(&self) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(self.text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// 取出 `path` 里所有带名字的 block，按名字排序。
pub fn blocks_in(path: &Path) -> Result<Vec<Block>, Error> {
    let file = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        file: file.clone(),
        source,
    })?;
    Ok(blocks(&text, &file))
}

/// 取出 `source` 里所有带名字的 block，`file` 只用于报错与记录来源。
pub fn blocks(source: &str, file: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut open: Option<(String, String, usize, String)> = None;
    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let mut fields = info.split_whitespace();
                if let (Some(lang), Some(name), None) =
                    (fields.next(), fields.next(), fields.next())
                {
                    let line = source[..range.start].lines().count() + 1;
                    open = Some((lang.to_owned(), name.to_owned(), line, String::new()));
                }
            }
            Event::Text(text) => {
                if let Some((_, _, _, body)) = &mut open {
                    body.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, name, line, text)) = open.take() {
                    out.push(Block {
                        name,
                        lang,
                        text,
                        file: file.to_owned(),
                        line,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// 取出检查与取用的对象里全部带名字的 block，按名字索引，重复即错。
pub fn docs(root: &Path) -> Result<BTreeMap<String, Block>, Error> {
    collect(crate::documents(root).map_err(|source| Error::Io {
        file: root.display().to_string(),
        source,
    })?)
}

/// 构建只读消费方列出的文档；目录递归收 Markdown，文件按给出的路径读。
pub fn sources(root: &Path, paths: &[&str]) -> Result<BTreeMap<String, Block>, Error> {
    let mut files = Vec::new();
    for path in paths {
        let path = root.join(path);
        if path.is_dir() {
            files.extend(
                crate::files_with_extension(&path, "md").map_err(|source| Error::Io {
                    file: path.display().to_string(),
                    source,
                })?,
            );
        } else {
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    collect(files)
}

fn collect(files: Vec<std::path::PathBuf>) -> Result<BTreeMap<String, Block>, Error> {
    let mut found: BTreeMap<String, Block> = BTreeMap::new();
    for path in files {
        for block in blocks_in(&path)? {
            match found.get(&block.name) {
                Some(_) => {
                    return Err(Error::Duplicate {
                        name: block.name,
                        file: block.file,
                        line: block.line,
                    });
                }
                None => {
                    found.insert(block.name.clone(), block);
                }
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\
# 文档

正文。

```json value.bundle
{\"type\": \"object\"}
```

```mermaid instance.status
stateDiagram-v2
  running --> paused : pause
```

```python
不带名字，不算。
```
";

    #[test]
    fn takes_named_blocks_with_language_and_line() {
        let found = blocks(SOURCE, "t.md");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "value.bundle");
        assert_eq!(found[0].lang, "json");
        assert_eq!(found[0].line, 5);
        assert!(found[0].text.contains("object"));
        assert_eq!(found[1].name, "instance.status");
        assert_eq!(found[1].lang, "mermaid");
    }

    #[test]
    fn a_block_without_a_name_is_ignored() {
        let found = blocks("```python\nx = 1\n```\n", "t.md");
        assert!(found.is_empty());
    }

    #[test]
    fn sha256_is_the_content_hash() {
        // 空内容的 SHA-256。
        let b = Block {
            name: "x".into(),
            lang: "json".into(),
            text: String::new(),
            file: "t.md".into(),
            line: 1,
        };
        assert_eq!(
            b.sha256(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn source_lists_isolate_blocks_and_require_declared_dependencies() {
        let root = std::env::temp_dir().join(format!("spec-sources-{}", std::process::id()));
        std::fs::create_dir_all(root.join("core")).unwrap();
        std::fs::create_dir_all(root.join("dependency")).unwrap();
        std::fs::write(
            root.join("core/main.md"),
            "```json core.shape\n{\"$block\":\"shared.shape\"}\n```\n",
        )
        .unwrap();
        std::fs::write(
            root.join("dependency/shared.md"),
            "```json shared.shape\n{\"type\":\"boolean\"}\n```\n",
        )
        .unwrap();
        // 清单外的同名 block 不影响本构建，缺少显式依赖时仍报错。
        std::fs::write(root.join("unrelated.md"), "```json core.shape\n{}\n```\n").unwrap();
        let core = sources(&root, &["core/", "core/main.md"]).unwrap();
        assert_eq!(core.len(), 1, "重叠的输入路径只读一次");
        assert!(crate::expand(&core).is_err());
        let complete = sources(&root, &["core/", "dependency/"]).unwrap();
        let expanded = crate::expand(&complete).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&expanded["core.shape"].text).unwrap(),
            serde_json::json!({"type":"boolean"})
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_repository_has_no_duplicate_block_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let found = docs(&root).unwrap();
        assert!(found.contains_key("value.schema"));
        assert!(found.contains_key("reference.kernel"));
    }
}
