//! 检查名词表里的名字只出现一次。
//! 参考：docs/README.md（名词表）。
//!
//! 表在「## 名词表」之下，头两行是表头与分隔行。每行前两列是英文名与中文名，一格里
//! 用「；」「/」分开几个名字；两列共用一个名字空间，按原文比较。

use std::collections::BTreeMap;
use std::path::Path;

/// 名词表所在的文件，相对仓库根。
const GLOSSARY: &str = "docs/README.md";
const HEADING: &str = "## 名词表";

/// 名词表里的一个名字，`line` 是它所在的行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlossaryName {
    pub name: String,
    pub line: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum GlossaryError {
    #[error("{file}:{line}: 名词表里的「{name}」与第 {first} 行重复")]
    Duplicate {
        file: String,
        line: usize,
        name: String,
        first: usize,
    },
    #[error("{file}: 找不到「{HEADING}」下的表")]
    Missing { file: String },
    #[error("{file}: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
}

/// 取出 `text` 里名词表前两列的名字，按出现的顺序；没有这张表时返回 `None`。
pub fn glossary_names(text: &str) -> Option<Vec<GlossaryName>> {
    let mut lines = text.lines().enumerate();
    lines.find(|(_, line)| line.trim() == HEADING)?;
    let rows: Vec<(usize, &str)> = lines
        .take_while(|(_, line)| !line.starts_with('#'))
        .skip_while(|(_, line)| !line.trim_start().starts_with('|'))
        .take_while(|(_, line)| line.trim_start().starts_with('|'))
        .collect();
    if rows.len() < 2 {
        return None;
    }
    let mut out = Vec::new();
    for (index, row) in rows.into_iter().skip(2) {
        let cells = row.trim().trim_start_matches('|').split('|').take(2);
        for name in cells.flat_map(|cell| cell.split(['；', '/'])) {
            let name = name.trim();
            if !name.is_empty() {
                out.push(GlossaryName {
                    name: name.to_owned(),
                    line: index + 1,
                });
            }
        }
    }
    Some(out)
}

/// 检查名词表。
pub fn check_glossary(root: &Path) -> Result<(), Vec<GlossaryError>> {
    let path = root.join(GLOSSARY);
    let file = path.display().to_string();
    let text = std::fs::read_to_string(&path).map_err(|source| {
        vec![GlossaryError::Io {
            file: file.clone(),
            source,
        }]
    })?;
    let names =
        glossary_names(&text).ok_or_else(|| vec![GlossaryError::Missing { file: file.clone() }])?;
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut errors = Vec::new();
    for GlossaryName { name, line } in &names {
        match seen.get(name.as_str()) {
            Some(&first) => errors.push(GlossaryError::Duplicate {
                file: file.clone(),
                line: *line,
                name: name.clone(),
                first,
            }),
            None => {
                seen.insert(name, *line);
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
# 总览

## 名词表

说明。

| English | 中文 | 正文 | 用法 |
|---|---|---|---|
| Port；Provided port / Required port | 端口；服务端口 / 需要端口 | 英 | |
| Value | 值 | 英 | 用法里的 Port 不算 |

## 别的
";

    #[test]
    fn names_split_on_semicolons_and_slashes() {
        let found = glossary_names(TABLE).unwrap();
        let names: Vec<&str> = found.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Port",
                "Provided port",
                "Required port",
                "端口",
                "服务端口",
                "需要端口",
                "Value",
                "值"
            ]
        );
        assert_eq!(found[0].line, 9);
        assert_eq!(found[6].line, 10);
    }

    #[test]
    fn a_repeated_name_is_reported_with_both_lines() {
        let root = std::env::temp_dir().join(format!("spec-glossary-{}", std::process::id()));
        std::fs::create_dir_all(root.join("docs")).unwrap();
        let text = TABLE.replace("| Value | 值 |", "| Value | 端口 |");
        std::fs::write(root.join(GLOSSARY), text).unwrap();
        let errors = check_glossary(&root).unwrap_err();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], GlossaryError::Duplicate { name, line: 10, first: 9, .. }
            if name == "端口")
        );
    }

    #[test]
    fn a_missing_table_is_an_error() {
        assert!(glossary_names("# 总览\n\n## 名词表\n\n没有表。\n").is_none());
        assert!(glossary_names("# 总览\n").is_none());
        assert!(glossary_names("## 名词表\n\n## 别的\n\n| a | b |\n|---|---|\n").is_none());
    }

    #[test]
    fn the_repository_glossary_has_unique_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        if let Err(errors) = check_glossary(&root) {
            let report: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            panic!("名词表有 {} 处重复：\n{}", report.len(), report.join("\n"));
        }
    }
}
