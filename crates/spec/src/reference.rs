//! 检查文档与 Rust 模块注释的「参考：」行，共用路径与节名校验。
//! 参考：docs/Kernel/Engineering.md（接口）。

use std::path::Path;

/// 「参考：」行里的一条引用。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub file: String,
    pub line: usize,
    /// 文档里相对所在目录，Rust 模块注释里相对仓库根。
    pub path: String,
    /// 括号里写的节名。
    pub sections: Vec<String>,
}

/// 一条参考解析不了：路径不存在，或括号里的节名在目标文件里找不到。
#[derive(Debug, thiserror::Error)]
pub enum ReferenceError {
    #[error("{file}:{line}: 参考行里的 `{path}` 解析不到")]
    Missing {
        file: String,
        line: usize,
        path: String,
    },
    #[error("{file}:{line}: 参考行里的 `{path}` 指向 {target}，那里找不到「{section}」")]
    Section {
        file: String,
        line: usize,
        path: String,
        target: String,
        section: String,
    },
    #[error("{file}: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
}

/// 取出 `text` 里所有「参考：」行的条目，`file` 只用于报错与记录来源。
pub fn references(text: &str, file: &str) -> Vec<Reference> {
    let mut out = Vec::new();
    for (line, text) in text.lines().enumerate() {
        let Some(rest) = text.trim().strip_prefix("参考：") else {
            continue;
        };
        for part in rest.split('；') {
            let part = part.trim().trim_end_matches('。');
            let (path, sections) = match part.split_once('（') {
                Some((path, rest)) => (
                    path.trim(),
                    rest.trim_end_matches('）')
                        .split('、')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect(),
                ),
                None => (part, Vec::new()),
            };
            out.push(Reference {
                file: file.to_owned(),
                line: line + 1,
                path: path.to_owned(),
                sections,
            });
        }
    }
    out
}

fn source_references(text: &str, file: &str) -> Vec<Reference> {
    // 只读文件开头的模块注释，代码与测试字符串里的伪注释不会成为引用。
    let header = text
        .lines()
        .map(str::trim_start)
        .take_while(|line| line.is_empty() || line.starts_with("//"))
        .map(|line| line.strip_prefix("//!").unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    references(&header, file)
}

fn validate_reference(base: &Path, r: Reference, errors: &mut Vec<ReferenceError>) {
    if !(r.path.ends_with(".md") || r.path.ends_with('/')) {
        return;
    }
    let target = base.join(&r.path);
    if !target.exists() {
        errors.push(ReferenceError::Missing {
            file: r.file,
            line: r.line,
            path: r.path,
        });
        return;
    }
    if r.sections.is_empty() {
        return;
    }
    let content = if target.is_dir() {
        target.join("README.md")
    } else {
        target
    };
    let text = match std::fs::read_to_string(&content) {
        Ok(text) => text,
        Err(source) => {
            errors.push(ReferenceError::Io {
                file: content.display().to_string(),
                source,
            });
            return;
        }
    };
    for section in r.sections {
        if !text.contains(&section) {
            errors.push(ReferenceError::Section {
                file: r.file.clone(),
                line: r.line,
                path: r.path.clone(),
                target: content.display().to_string(),
                section,
            });
        }
    }
}

/// 文档从所在目录解析，Rust 模块注释从仓库根解析。
pub fn check_references(root: &Path) -> Result<(), Vec<ReferenceError>> {
    let files = (|| -> std::io::Result<_> {
        let mut files: Vec<_> = crate::documents(root)?
            .into_iter()
            .map(|p| (p, false))
            .collect();
        let crates = root.join("crates");
        if crates.exists() {
            files.extend(
                crate::files_with_extension(&crates, "rs")?
                    .into_iter()
                    .map(|p| (p, true)),
            );
        }
        Ok(files)
    })();
    let files = match files {
        Ok(files) => files,
        Err(source) => {
            return Err(vec![ReferenceError::Io {
                file: root.display().to_string(),
                source,
            }]);
        }
    };
    let mut errors = Vec::new();
    for (path, source_file) in files {
        let file = path.display().to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => {
                errors.push(ReferenceError::Io { file, source });
                continue;
            }
        };
        let (base, refs) = if source_file {
            (root, source_references(&text, &file))
        } else {
            (path.parent().unwrap_or(root), references(&text, &file))
        };
        for r in refs {
            validate_reference(base, r, &mut errors);
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

    #[test]
    fn references_carry_paths_and_sections() {
        let found = references(
            "正文。\n参考：README.md（Interface、Port）；../Value.md；Definition/（Charter）。\n",
            "t.md",
        );
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].path, "README.md");
        assert_eq!(found[0].sections, ["Interface", "Port"]);
        assert_eq!(found[0].line, 2);
        assert!(found[1].sections.is_empty());
        assert_eq!(found[2].path, "Definition/");
    }

    #[test]
    fn a_broken_reference_is_reported() {
        let root = std::env::temp_dir().join(format!("spec-refs-{}", std::process::id()));
        let docs = root.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("a.md"), "参考：b.md（没有这一节）；c.md。\n").unwrap();
        std::fs::write(docs.join("b.md"), "# 有一节\n").unwrap();
        let errors = check_references(&root).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(
            matches!(&errors[0], ReferenceError::Section { path, section, .. }
                if path == "b.md" && section == "没有这一节")
        );
        assert!(matches!(&errors[1], ReferenceError::Missing { path, .. } if path == "c.md"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn source_headers_preserve_lines_and_ignore_code() {
        let text = "// license\n//! 模块。\n//! 参考：docs/a.md（甲、乙）；AGENTS.md（检查）。\n\nfn f() {}\n//! 参考：missing.md。\n";
        let found = source_references(text, "src/lib.rs");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].path, "docs/a.md");
        assert_eq!(found[0].sections, ["甲", "乙"]);
        assert_eq!(found[0].line, 3);
        assert!(
            source_references("const S: &str = r#\"\n//! 参考：missing.md。\n\"#;", "t.rs")
                .is_empty()
        );
    }

    #[test]
    fn source_paths_are_root_relative_and_broken_sections_fail() {
        let root = std::env::temp_dir().join(format!("spec-source-refs-{}", std::process::id()));
        std::fs::create_dir_all(root.join("docs/part")).unwrap();
        std::fs::create_dir_all(root.join("crates/example/src")).unwrap();
        std::fs::write(root.join("docs/part/README.md"), "# 已有\n").unwrap();
        let source = root.join("crates/example/src/lib.rs");
        std::fs::write(&source, "//! 参考：docs/part/（已有）。\nfn f() {}").unwrap();
        check_references(&root).unwrap();
        std::fs::write(
            &source,
            "//! 模块\n//! 参考：docs/part/README.md（失效）；docs/missing.md。",
        )
        .unwrap();
        let errors = check_references(&root).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(
            matches!(&errors[0], ReferenceError::Section { section, line: 2, .. } if section == "失效")
        );
        assert!(
            matches!(&errors[1], ReferenceError::Missing { path, line: 2, .. } if path == "docs/missing.md")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn every_reference_resolves() {
        let root = crate::generate::find_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("find root");
        if let Err(errors) = check_references(&root) {
            let report: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            panic!("{} 条参考解析不了：\n{}", report.len(), report.join("\n"));
        }
    }
}
